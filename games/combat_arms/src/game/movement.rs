//! Combat Arms' player movement, from specs/combat_arms/movement.md: the
//! movement slot's implementation `combat_arms:movement` (docs/DESIGN.md,
//! "Slots"), the loadout's default. LithTech "player physics": each tick
//! the horizontal velocity is clamped to the stance's speed limit, an
//! acceleration is built from the held keys (ramped from a start value),
//! and both are integrated in closed form with exponential ground friction
//! (gravity in the air, quarter-step displacement without friction).
//! Crouching blends the speed limit and the hull height by a crouch rate
//! (with the crouch-spam slowdown and lock-out); sprint drains a stamina
//! gauge; jumps set the vertical speed and leave a jump factor that weakens
//! the next jump and the air acceleration; landings from high enough deal
//! fall damage (`core::Damage`, crush, from the player).
//!
//! The maths runs in world units (u, +Y up, as the spec); the character's
//! `Transform` (engine meters) is the hull's centre, `meters_per_unit`
//! apart (data/movement.toml). The hull is an upright cylinder of radius
//! `max(halfX, halfZ)` and the crouch-blended half-height; the entity's
//! `Collider` follows it (`sync_collider`). Collision is avian shape casts
//! (`MoveAndSlide::cast_move`, keeping a 0.1 u skin): slide along the
//! first surface (pushed 0.1 % off it), then the crease of two, stopping
//! at a third; only the level part of a move up a steep floor; stair-step
//! up to 30.5 u; a 1 u (0.02 x halfY) standing probe below the hull. Parry's cylinder casts stop short of flat faces on long
//! sweeps and report slightly tilted normals, so `Space::cast` sweeps a
//! hit again from where it stopped until it settles, and takes the
//! surface's normal from a short ray at the contact.
//!
//! Numbers: data/movement.toml (`MovementData`, spec values marked by
//! section). Per player: `CaCharacter` (figure and gear: sprint, stamina,
//! penalties; the reference character by default) and `HeldMovePenalty`
//! (the held weapon's `MovePenalty`, set by the weapon part; none: 0);
//! `StunPenalty` (P_stun, from a stun bullet: `attachments`). The HUD
//! reads the stamina gauge and the crouch lock-out from `CaMovement`.
//! The eye sits `eye_above_centre` over the hull's centre (83 u above the
//! feet standing, 60 crouched); `CameraRise` is the camera's upward
//! smoothing (the front end's `ui::effects::camera_motion` applies it to
//! the first-person camera each rendered frame). An infected character
//! (`CaInfected`, the infection modes' rules set it) hit by the other team
//! gets a `HitKnockback` from the weapon part (its ammo's
//! `DamageMovePenalty`): the push replaces the acceleration while it runs.
//! A spawn or respawn puts the player on the floor below (or on a liquid
//! volume's top, when one is on the way down: ours) and measures falls
//! from there; a teleport resets nothing.
//!
//! Volumes (spec "Volumes (containers)"; the map's `volumes::CaVolume`s):
//! a volume acts when the hull's box touches it; the eye point (the
//! camera, from last tick's centre) gives the eye volume. On a ladder
//! (the box touching a visible `Ladder`) gravity is off, also the tick
//! after leaving it, and with it every ground rule of the integrator, so
//! a player at a ladder's foot looking up climbs from the floor; only the
//! upward speed is capped (200, no horizontal limit), forward is the
//! camera's full direction and crouch climbs down at -3000; no jump,
//! sprint or fall damage. Free movement (the eye in a liquid or a
//! ladder, or the box on a ladder): swimming forward is the camera's
//! direction without its upward part, crouch sinks at -1500, a jump from
//! the bottom pushes +3000 on its tick, the crouch rate is held at 0, and
//! when the eye's free movement ends the player is stopped once; the
//! camera's rise waits while the eye is in one. The first ladder's and
//! every other visible volume's viscosity takes `viscosity_max` x it x dt
//! off speeds above 1 u/s (not in a jump, except on a ladder), every
//! current adds current x dt, and their sums stop the braking; the ground
//! friction is 5 x the least `Friction` touched (hidden ones too, not
//! filters, nets or pads; 0: the open form); the speed limit is
//! multiplied by their `PlayerSpeedFactor`s; gravity volumes replace the
//! gravity by the sum of their `PlayerGravity`; a jump pad (`JumpVolume`,
//! switched off too) launches a player who stood on ground last tick,
//! without the key, adding its velocity along its rotation's forward;
//! landing with the centre in a safety net (`SafteyNet`, no physics)
//! does no fall damage. Pushers are `CaPusher`s (the door kick); a bomb
//! plant (`rules::objectives::CaAction`) or `MovementLocked` disallows
//! movement (damped, the keys ignored; a jump pressed then waits); a
//! game mode's `ModeSpeed` scales the limit and the stamina capacity.
//! A jump that can't happen yet stays pending until the next ground.
//! The gravity is also the server setting `combat_arms:sv_gravity`
//! (`GRAVITY_CVAR`, ours: replicated, so a client predicts with the
//! server's, a party host's moon gravity).
//!
//! Known shortcuts (docs/tech-debt.md): Combat Arms movement
//! (specs/combat_arms/movement.md) runs once per engine tick, the game's
//! tick set to the spec's 15 ms (`game::TICK_INTERVAL`); the spec's own
//! frame accumulator (at most 6 ticks per frame, the rest dropped) is
//! `TickClock`, not wired to the engine's fixed clock. So what the spec
//! runs per rendered frame runs per tick here: the stamina gauge, the eye
//! volume, and the crouch release delay is checked against the tick's
//! time, not the frame's (the same totals; edges a frame apart). The
//! server re-runs the movement from the inputs and the client predicts
//! (the spec's server moves by the client's displacements): sprint
//! applies at once (the listen-server case), and the gauge's server
//! stamina steps are our ticks, each at most `stamina_step_max`. The hull
//! box is tested against a volume's bounds and face planes (exact for the
//! convex brushes; no edge axes), once a tick with the hull's height
//! before that tick's resize; of overlapping eye volumes a liquid or a
//! ladder wins (`volumes::eye_volume`; the spec's order is unknown); the
//! integrator's flat normal on a model is for characters only (props
//! and moving world models give the contact's normal); the steep-floor cut uses what the last move stood on, not a
//! new probe; a blocked move is one that moved less than 0.01 u (the
//! mover's clearance jitter; the spec's 10^-6); a locked tick in a
//! gravity volume uses the last free tick's gravity. `viscosity_max` is
//! grenades.md's 5000. The custom term P_custom is `CustomMovePenalty`
//! (Hired Guns' carry penalty) and a VIP account `VipMovement` (VIP
//! Escort's VIP: its model's penalty alone, the figure's stamina; its
//! sprint keeps the figure's dash speed). Not modelled: kill-streak and
//! stamina perks, the ammo's own stun, the "falling" sound status,
//! stamina kits, jump stamina, the modes' actual `ModeSpeed` values
//! (game-mode specs), maps' not-a-step surface flag (characters and
//! moving bodies are not steps). The shape cast reports where it
//! touched, not which cap: a stair-step drop or a standing probe that
//! touches an edge above the hull's flat bottom is taken as the curved
//! side (a level normal). Scale: the spec stays in world units and the
//! game calls 40 u a metre for its HUD readouts (`hud_metres`), but its
//! bodies aren't built to that (a 100 u hull would be 2.5 m), so
//! `meters_per_unit` scales by body size instead (open question 3; to be
//! checked live). Why it's OK for now: every spec test case is a scenario
//! test on the real collision (games/combat_arms/tests/it/movement.rs,
//! the volumes' in movement_volumes.rs, the corrections' in
//! movement_corrections.rs) and passes within the stated tolerances; the
//! volumes' readings, a ladder climbed from the deck and swimming are
//! checked on the install's maps (`heavy::movement_volumes`); the missing
//! pieces have nothing in Chimera's Combat Arms to act on yet.

use std::sync::OnceLock;

use avian3d::{character_controller::move_and_slide::DepenetrationConfig, prelude::*};
use bevy::prelude::*;
use chimera_core::console::ConsoleAppExt;
use chimera_sim::server_docs::DocsAppExt;
use serde::{Deserialize, Serialize};

use super::{
    rules::objectives::{ActionKind, CaAction},
    volumes::CaVolume,
};
use crate::{
    character::{CAPSULE_HEIGHT, CAPSULE_RADIUS},
    core::{
        CHARACTER_FILTER, Damage, DamageKind, Dead, FirstTimePredicted, Hitgroup, Intent, MovementState, Predict,
        PredictedAppExt, SimClock, Velocity,
    },
    slots::{Footing, Loadout, RegisterSlots},
    weapon::Zoomed,
};

/// The movement Combat Arms' players get.
pub const ID: &str = "combat_arms:movement";

/// Ours: how far below a spawn point the floor is looked for (u).
const SPAWN_DROP: f32 = 100_000.0;

/// data/movement.toml, built in.
pub const MOVEMENT_DATA: &str = include_str!("../../data/movement.toml");

/// The movement's numbers (data/movement.toml; spec "Constants").
#[derive(Resource, Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MovementData {
    pub meters_per_unit: f32,
    pub skin: f32,
    pub eye_offset: f32,
    pub eye_crouch_offset: f32,
    pub camera_rise_time: f32,
    pub camera_rise_min_speed: f32,
    pub camera_rise_max_lag: f32,
    pub hud_units_per_metre: f32,
    pub run_forward: f32,
    pub run_back: f32,
    pub run_strafe: f32,
    pub walk: f32,
    pub crouch: f32,
    pub jump: f32,
    pub ladder: f32,
    pub gravity: f32,
    pub friction: f32,
    pub base_accel: f32,
    pub start_accel: f32,
    pub max_accel: f32,
    pub accel_ramp: f32,
    pub crouch_accel_factor: f32,
    pub in_air_accel_factor: f32,
    pub slide_to_stop_time: f32,
    pub stop_snap_speed: f32,
    pub rest_threshold: f32,
    pub jump_factor_after_jump: f32,
    pub jump_factor_recovery: f32,
    pub jump_stance_tolerance: f32,
    pub walkable_normal: f32,
    pub on_ground_normal: f32,
    pub stair_step: f32,
    pub standing_probe: f32,
    pub stand_half_extents: [f32; 3],
    pub crouch_half_extents: [f32; 3],
    pub crouch_release_delay: f64,
    pub fall_damage_start: f32,
    pub fall_damage_full: f32,
    pub fall_damage_at_full: f32,
    pub fall_minimum: f32,
    pub health_points: f32,
    pub viscosity_max: f32,
    pub viscosity_min_speed: f32,
    pub stamina_step_max: f32,
    pub pusher_air_factor: f32,
    pub pusher_max_up: f32,
    pub crouch_spam: CrouchSpamData,
    pub character: CaCharacter,
    /// The body types (`FigureType#`) by index, the install's (gear_perks.md
    /// 2: a vest record's `FigureType` picks one); empty, or an index past
    /// the list: `character.figure` (the reference body, `FigureType0`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub figures: Vec<Figure>,
}

impl MovementData {
    /// Body type `figure`'s numbers (`figures`, else the reference body's).
    pub fn figure_of(&self, figure: usize) -> &Figure {
        self.figures.get(figure).unwrap_or(&self.character.figure)
    }
}

/// The crouch-spam penalty's parameters (spec "Crouch-spam penalty").
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CrouchSpamData {
    pub duration: f32,
    pub tangents: [f32; 2],
    pub enable_slowdown: bool,
    pub slowdown_increase_duration: f32,
    pub slowdown_increase_tangents: [f32; 2],
    pub slowdown_decrease_amount: f32,
    pub max_slowdown_rate: f32,
    pub min_slowdown_sp: f32,
    pub enable_lockout: bool,
    pub lockout_increase_duration: f32,
    pub lockout_increase_tangents: [f32; 2],
    pub lockout_decrease_amount: f32,
    pub lockout_decrease_amount_in_penalty: f32,
    pub decrease_during_jumping_crouch: bool,
    pub decrease_by_keystroke: bool,
    pub decrease_exception_no_movement: bool,
    pub no_movement_time_limit: f32,
    pub no_movement_deviation_limit: f32,
}

/// A player's figure type and worn gear: what the speed limit, sprint and
/// stamina read (spec "Character and weapon data"). Absent on a player:
/// the data's (the reference character).
#[derive(Component, Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CaCharacter {
    pub figure: Figure,
    #[serde(default)]
    pub gear: Vec<Gear>,
    /// Fighting Chance's factors (gear_perks.md 15) on the speed limit
    /// and the stamina capacity (1: none).
    #[serde(default = "one")]
    pub speed_scale: f32,
    #[serde(default = "one")]
    pub stamina_scale: f32,
}

fn one() -> f32 {
    1.0
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Figure {
    /// `DashSpeed`: sprint multiplier on the run speed.
    pub dash_speed: f32,
    /// `DashGauge`: stamina drained per second sprinting.
    pub dash_gauge: f32,
    /// `GaugeRec`: stamina recovered per second.
    pub gauge_recovery: f32,
    /// `Maxstamina`.
    pub max_stamina: f32,
    /// `VariableSpeed`, `VariableWeight`, `VariableJump`.
    pub variable_speed: f32,
    pub variable_weight: f32,
    pub variable_jump: f32,
    /// `StiffDuration`: the hit knock-back lasts `1 + this` times longer.
    #[serde(default)]
    pub stiff_duration: f32,
}

/// One worn item (vest, head piece, goggles, backpack, gear mod).
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Gear {
    pub name: String,
    /// `DashSpeed`: added to the sprint bonus sum.
    pub dash_speed: f32,
    /// `MaxStamina`: added to the capacity bonus sum.
    pub max_stamina: f32,
    /// `MovePenalty`: added to the penalty.
    pub move_penalty: f32,
    /// Jump bonus (`J_model`).
    pub jump: f32,
}

impl CaCharacter {
    /// No gear, figure values as given (the spec's test cases' "no gear
    /// bonuses").
    pub fn without_gear(&self) -> Self {
        Self {
            figure: self.figure.clone(),
            gear: Vec::new(),
            speed_scale: 1.0,
            stamina_scale: 1.0,
        }
    }

    /// The VIP of VIP Escort (gear_perks.md 10): sprint is the run
    /// speed times one plus the vest's bonus alone (no figure multiplier,
    /// no other item), the stamina the figure's alone.
    pub fn vip(&self) -> Self {
        let mut figure = self.figure.clone();
        figure.dash_speed = 1.0;
        Self {
            figure,
            gear: self
                .gear
                .first()
                .map(|vest| Gear {
                    max_stamina: 0.0,
                    ..vest.clone()
                })
                .into_iter()
                .collect(),
            speed_scale: 1.0,
            stamina_scale: 1.0,
        }
    }

    fn gear_sum(&self, f: impl Fn(&Gear) -> f32) -> f32 {
        self.gear.iter().map(f).sum()
    }

    /// The stamina capacity: floor(Maxstamina x (1 + bonuses)).
    pub fn stamina_max(&self) -> f32 {
        (self.figure.max_stamina * (1.0 + self.gear_sum(|g| g.max_stamina)) * self.stamina_scale).floor()
    }
}

/// The held weapon's `MovePenalty` and its modification's factor
/// (`c_mod`), on the player. Set by the weapon part; on every character
/// (`weapons`), its default no penalty.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
#[component(storage = "SparseSet")]
pub struct HeldMovePenalty {
    pub penalty: f32,
    pub mod_factor: f32,
}

/// A stun's slowdown (spec "Penalty", P_stun): added to the penalty until
/// `until` (game time, s). Set by the shooter's stun bullet
/// (`attachments`); predicted with the victim's movement.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct StunPenalty {
    pub penalty: f32,
    pub until: f64,
}

/// The custom move penalty (P_custom, spec "Penalty"): Hired Guns' carry
/// penalty (game_modes_2.md M11.4), set by the rules; predicted with the
/// player's movement.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CustomMovePenalty(pub f32);

/// A VIP (spec "Penalty": a VIP account; game_modes_2.md M13.4): the
/// penalty is this alone (the VIP model's `MovePenalty`), whatever the
/// weapon, gear or custom term, and the stamina the figure's with no gear
/// bonuses. Set by VIP Escort's rules; predicted with the movement.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct VipMovement {
    pub penalty: f32,
}

/// An infected character of the infection mode: hits from the other
/// team knock it back (spec "Other modifiers", `HitKnockback`). The
/// infection modes' rules set it (`rules::quarantine`).
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct CaInfected;

/// A running hit knock-back (spec "Other modifiers", "Hit knock-back"):
/// set on an infected character by the weapon that hit it (its ammo's
/// `DamageMovePenalty` K and `DamageMovePenaltyDuration`).
#[derive(Component, Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct HitKnockback {
    /// The strength K: the push starts at K^2 u/s^2.
    pub strength: f32,
    /// The last hit's direction (the way the damage travelled), engine
    /// space; its horizontal part is pushed along.
    pub dir: Vec3,
    /// How long it lasts, s (the duration x (1 + `StiffDuration`)).
    pub duration: f32,
    /// When the movement first saw it (its t = 0); None until then.
    pub started: Option<f64>,
}

impl HitKnockback {
    /// A hit with knock-back `strength` for `duration` s on a figure with
    /// `stiff` (`StiffDuration`) at `now`, over the one running (`current`):
    /// the new timer replaces it only when longer than the time it has
    /// left; the direction is always the latest hit's. None: no knock-back
    /// (both values must be above 0).
    pub fn hit(current: Option<&Self>, strength: f32, duration: f32, stiff: f32, dir: Vec3, now: f64) -> Option<Self> {
        if strength <= 0.0 || duration <= 0.0 {
            return current.copied();
        }
        let new = Self {
            strength,
            dir,
            duration: duration * (1.0 + stiff),
            started: None,
        };
        Some(match current {
            Some(c) => {
                let left = c.started.map_or(c.duration as f64, |s| c.duration as f64 - (now - s)) as f32;
                if new.duration > left { new } else { Self { dir, ..*c } }
            }
            None => new,
        })
    }

    /// Seconds since it started at `now` while it runs; None when over.
    fn running(&self, now: f64) -> Option<f32> {
        let t = (now - self.started.unwrap_or(now)) as f32;
        (t < self.duration - 1e-6).then_some(t)
    }
}

/// A pusher (spec "Other modifiers", "Pushers"), added at game time
/// `added`: after its `delay` (s) has counted down by the tick, on every
/// tick of its `duration` (s), a player whose centre is within `radius`
/// (u) of `origin` (engine meters) with a clear line from it (the world
/// only) gets `strength x (1 - distance / radius)` u/s along that line
/// added to its velocity, a tenth (`pusher_air_factor`) standing on
/// nothing; then an upward speed above `pusher_max_up` is cut to it. It
/// is despawned when its time is up. The only working one is an AI's
/// door kick (`door_kick`); the bullets' and rockets' records have radius
/// 0 and never push.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct CaPusher {
    pub origin: Vec3,
    pub radius: f32,
    pub strength: f32,
    pub added: f64,
    pub delay: f32,
    pub duration: f32,
}

/// Where a pusher is in its life on a tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PusherPhase {
    /// Its delay is still counting down (or it was added this tick).
    Waiting,
    Active,
    /// Its time is up: removed.
    Over,
}

impl CaPusher {
    /// An AI's door kick at `origin` (engine meters), added at `now`:
    /// radius 96, strength 1000, no delay, 1 s.
    pub fn door_kick(origin: Vec3, now: f64) -> Self {
        Self {
            origin,
            radius: 96.0,
            strength: 1000.0,
            added: now,
            delay: 0.0,
            duration: 1.0,
        }
    }

    /// Its phase on the tick at `now` (ticks of `dt` s): the delay counts
    /// down by `dt` a tick, then on the tick it is used up and after, the
    /// time left; it pushes while that is above 0.
    pub fn phase(&self, now: f64, dt: f32) -> PusherPhase {
        let k = ((now - self.added) / dt as f64).round() as i64;
        let k0 = ((self.delay / dt) - 1e-4).ceil().max(1.0) as i64;
        if k < k0 {
            return PusherPhase::Waiting;
        }
        let left = self.duration - (k - k0 + 1) as f32 * dt;
        if left > 1e-5 { PusherPhase::Active } else { PusherPhase::Over }
    }
}

/// Movement disallowed (spec "Other modifiers", "Movement disallowed";
/// Corrections 9): the movement keys set nothing and the motion step only
/// damps. Set while planting the bomb (the movement also reads the
/// planter's `rules::objectives::CaAction`), or by anything else that
/// locks a player in place.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct MovementLocked;

/// A game mode's factors on a player's speed limit and stamina capacity
/// (spec "Speed limit", Corrections 21: a mutation item's buff or nerf,
/// the infected mercenary's speed rate against humans): 1 without one.
/// The values are the game-mode specs'.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct ModeSpeed {
    pub speed: f32,
    pub stamina: f32,
}

impl Default for ModeSpeed {
    fn default() -> Self {
        Self { speed: 1.0, stamina: 1.0 }
    }
}

/// The first-person camera's upward smoothing (spec "Eye (camera)
/// height"): rising more than 1 u (not in a jump) it moves at
/// `max(gap / 0.1 s, 140 u/s)`, at most 30 u behind, and snaps once
/// within 1 u; downward moves are immediate. Heights in u. The client's
/// camera runs it each rendered frame (`ui::effects::camera_motion`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CameraRise {
    pub height: Option<f32>,
}

impl CameraRise {
    /// One rendered frame of `dt` s toward the wanted height: the camera's.
    pub fn step(&mut self, wanted: f32, dt: f32, in_jump: bool, data: &MovementData) -> f32 {
        let h = match self.height {
            Some(h) if wanted - h > 1.0 && !in_jump => {
                let h = h.max(wanted - data.camera_rise_max_lag);
                let speed = ((wanted - h) / data.camera_rise_time).max(data.camera_rise_min_speed);
                let h = (h + speed * dt).min(wanted);
                if wanted - h <= 1.0 { wanted } else { h }
            }
            _ => wanted,
        };
        self.height = Some(h);
        h
    }
}

/// The eye's height above the hull's centre (u) at crouch rate `r` (spec
/// "Eye (camera) height"): fixed from the standing box, plus the crouch
/// offset.
pub fn eye_above_centre(r: f32, data: &MovementData) -> f32 {
    data.eye_offset * data.stand_half_extents[1] + data.eye_crouch_offset * r
}

/// A distance as the game's HUD readouts print it: whole "metres" of 40 u
/// (spec "Units and conventions").
pub fn hud_metres(units: f32, data: &MovementData) -> u32 {
    (units / data.hud_units_per_metre).floor().max(0.0) as u32
}

/// The built-in movement data (a read-only table, parsed once; the
/// install's body type is over it in `CaData::movement`, which the app's
/// `MovementData` resource holds). Bad data panics (the tests load it).
pub fn movement_data() -> &'static MovementData {
    static DATA: OnceLock<MovementData> = OnceLock::new();
    DATA.get_or_init(|| {
        toml::from_str(MOVEMENT_DATA).unwrap_or_else(|e| panic!("data/movement.toml: {e}"))
    })
}

/// Combat Arms' movement: registered as `ID` and made the loadout's
/// default.
pub struct CaMovementPlugin;

/// `MovementData::gravity` (its magnitude, u/s² downward) as a server
/// setting: `sv_` makes it replicated (`console::CvarScope`), so a
/// network client predicts its own player with the server's gravity, as
/// a Source movement's `sv_gravity`. Not the original's (the spec has no
/// such setting; its gravity is the data's); ours for a party host's
/// moon gravity (`party_kit::surprise`).
pub const GRAVITY_CVAR: &str = "combat_arms:sv_gravity";

impl Plugin for CaMovementPlugin {
    fn build(&self, app: &mut App) {
        let movement = super::data::CaData::of(app).movement().clone();
        // A network client whose movement numbers differ takes the server's.
        app.server_docs(
            super::data::served::MOVEMENT,
            super::data::served::movement_doc(&movement),
            chimera_sim::server_docs::DocApply::With(super::data::served::apply_movement),
        );
        let gravity = format!("{}", -movement.gravity);
        // The gravity as a replicated server setting (`GRAVITY_CVAR`): a
        // client predicts its player with the server's (a party host's
        // moon gravity, `party_kit::surprise`).
        app.console_cvar(
            GRAVITY_CVAR,
            "Combat Arms' movement gravity, u/s^2 downward (the server's; its clients predict with it).",
            &gravity,
            |w| w.get_resource::<MovementData>().map(|d| format!("{}", -d.gravity)),
            |w, v| {
                let g: f32 = v.trim().parse().map_err(|_| format!("not a number: {v}"))?;
                if !g.is_finite() {
                    return Err(format!("not a gravity: {v}"));
                }
                w.get_resource_mut::<MovementData>().ok_or("no Combat Arms movement")?.gravity = -g;
                Ok(())
            },
        );
        app.insert_resource(movement)
            .insert_resource(Loadout { movement: ID })
            .register_movement::<CaMovement>(ID)
            .seat_movement(ID, footing, seat)
            .add_message::<super::volumes::PadLaunch>()
            .add_systems(Predict::Movement, (step, sync_collider).chain())
            .predicted_net::<CaMovement>()
            .predicted_net::<CustomMovePenalty>()
            .predicted_net::<VipMovement>();
    }
}

/// The stamina gauge's state (spec "Sprint and stamina").
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum StaminaState {
    #[default]
    Recovering,
    Draining,
    /// Sprint held with an empty gauge: pinned at 0.
    Stopped,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
enum CrouchPhase {
    #[default]
    Idle,
    Down,
    Up,
}

/// What the player stands on (from the last move).
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Standing {
    pub normal: Vec3,
    /// Another character (never ground for movement).
    pub character: bool,
}

/// Combat Arms' movement state on a player: the marker the slot inserts
/// and everything the movement keeps between ticks. Units are world units
/// (u, u/s). Public for the HUD (stamina, lock-out) and the tests.
#[derive(Component, Clone, Debug, Default, Serialize, Deserialize)]
#[component(storage = "SparseSet")]
pub struct CaMovement {
    // Control flags (spec "Input to control state"), this tick's.
    pub crouch: bool,
    pub run: bool,
    pub jump: bool,
    pub forward: bool,
    pub back: bool,
    pub left: bool,
    pub right: bool,
    pub moving: bool,
    pub sprint: bool,
    /// The crouch flag after the release and headroom holds, before the
    /// lock-out clears it: its turning on is a crouch press.
    crouch_held: bool,
    crouch_pressed_at: f64,
    jump_used: bool,
    /// A jump request waiting for ground (spec "Jumping", "Pending
    /// request"): cleared by the jump, by free movement and on a respawn.
    pub jump_pending: bool,
    /// In a jump (from the jump tick to the landing).
    pub in_jump: bool,
    /// The jump factor `j`, 1 at rest.
    pub jump_factor: f32,
    /// On ground for movement (walkable, not a character, not in a jump).
    pub on_ground: bool,
    /// What the hull rests on after the last move.
    pub standing: Option<Standing>,
    /// The crouch rate `r` (0 standing, 1 crouched).
    pub crouch_rate: f32,
    crouch_phase: CrouchPhase,
    crouch_elapsed: f32,
    /// The hull's half-height now (u; 0 before the first tick).
    pub half_height: f32,
    // Acceleration ramp.
    ramp_forward: f32,
    ramp_strafe: f32,
    /// The last direction speed `K`.
    pub direction_speed: f32,
    /// This tick's speed limit `V` and acceleration (u/s, u/s^2).
    pub speed_limit: f32,
    pub accel: Vec3,
    /// Stamina (spec "Sprint and stamina").
    pub stamina: f32,
    pub stamina_state: StaminaState,
    /// Crouch-spam values: slowdown and lock-out stamina, lock-out active.
    pub slowdown: f32,
    pub lockout: f32,
    pub crouch_locked: bool,
    spam_rising: bool,
    spam_elapsed: f32,
    /// Horizontal positions of the last ticks (time, x, z) for "still".
    history: Vec<(f64, Vec2)>,
    /// Highest Y while not standing (fall damage), u.
    fall_top: Option<f32>,
    /// Dead last tick (a respawn puts the player on the floor again).
    was_dead: bool,
    /// The hull touches a visible `Ladder` this tick (spec "Volumes").
    pub on_ladder: bool,
    /// The eye volume (spec "Volumes", "Membership") is a liquid this
    /// tick.
    pub in_liquid: bool,
    /// The eye volume is a liquid or a ladder this tick (the camera's
    /// rise waits; leaving it stops the player).
    pub eye_free: bool,
    /// Free movement this tick: `eye_free`, or the hull on a ladder.
    pub free_movement: bool,
    /// Movement disallowed this tick (`MovementLocked`, a bomb plant).
    pub locked: bool,
    /// The footing test failed this tick (characters.md Corrections 3):
    /// not on the ground, not jumped, not in free movement, and no floor
    /// within the footing ray (150 u) below the hull centre.
    pub no_footing: bool,
    /// What the integrator used last tick (gravity included), the
    /// gravity and ground friction, and whether the volumes stopped the
    /// braking: a locked tick reuses them.
    applied_accel: Vec3,
    last_gravity: f32,
    /// This tick's ground friction coefficient k (1/s), from the volumes.
    pub friction_k: f32,
    brake_blocked: bool,
    initialized: bool,
    /// Just switched to from another movement (`seat`): this tick's step
    /// first frees the hull and, when it stood on the ground (true), puts
    /// it down on the floor below.
    #[serde(default)]
    pub seat: Option<bool>,
}

impl CaMovement {
    /// The stamina as the HUD shows it (whole points).
    pub fn stamina_points(&self) -> f32 {
        self.stamina.floor()
    }
}

/// Where a Combat Arms character stands (`slots::Footing`): its hull's
/// bottom, crouched from half way down.
fn footing(world: &World, e: Entity) -> Option<Footing> {
    let me = world.get::<CaMovement>(e)?;
    let t = world.get::<Transform>(e)?;
    let d = world.get_resource::<MovementData>()?;
    let h = if me.half_height > 0.0 { me.half_height } else { d.stand_half_extents[1] };
    Some(Footing {
        feet: t.translation - Vec3::Y * h * d.meters_per_unit,
        crouched: me.crouch_rate >= 0.5,
        on_ground: me.on_ground || me.seat == Some(true),
    })
}

/// A character switched to Combat Arms' movement (`slots::set_movement`)
/// put on the old movement's feet, in the crouched hull if it crouched
/// (let go, it stands when there's room); its fall counts from there. The
/// step frees and grounds it (`reseat`).
fn seat(world: &mut World, e: Entity, f: Footing) {
    let Some(d) = world.get_resource::<MovementData>() else { return };
    let s = d.meters_per_unit;
    let h = if f.crouched { d.crouch_half_extents[1] } else { d.stand_half_extents[1] };
    let at = f.feet + Vec3::Y * h * s;
    let Ok(mut ent) = world.get_entity_mut(e) else { return };
    if let Some(mut me) = ent.get_mut::<CaMovement>() {
        me.half_height = h;
        me.crouch_rate = if f.crouched { 1.0 } else { 0.0 };
        me.crouch_held = f.crouched;
        me.crouch = f.crouched;
        me.fall_top = Some(at.y / s);
        me.seat = Some(f.on_ground);
    }
    if let Some(mut t) = ent.get_mut::<Transform>() {
        t.translation = at;
    }
    if let Some(mut p) = ent.get_mut::<Position>() {
        p.0 = at;
    }
}

/// After a switch from another movement (`seat`): a standing hull that
/// doesn't fit crouches (feet kept); one still overlapping something goes
/// to the nearest free place up to a stair step up or a radius aside;
/// then, if it stood on the ground, down onto a walkable floor within a
/// stair step below. Its fall counts from there.
fn reseat(space: &Space, d: &MovementData, me: &mut CaMovement, mut pos: Vec3, on_ground: bool) -> Vec3 {
    let crouch_h = d.crouch_half_extents[1];
    if me.half_height > crouch_h && !space.fits(pos, me.half_height) {
        let low = Vec3::new(pos.x, pos.y - me.half_height + crouch_h, pos.z);
        if space.fits(low, crouch_h) {
            pos = low;
            me.half_height = crouch_h;
            me.crouch_rate = 1.0;
            me.crouch_held = true;
            me.crouch = true;
        }
    }
    if !space.fits(pos, me.half_height) {
        const D: f32 = std::f32::consts::FRAC_1_SQRT_2;
        let dirs = [(1.0, 0.0), (D, D), (0.0, 1.0), (-D, D), (-1.0, 0.0), (-D, -D), (0.0, -1.0), (D, -D)];
        let r = space.radius;
        let mut offsets = Vec::new();
        for y in [0.0, 0.05, 0.1, 0.25, 0.5, 1.0].map(|k: f32| k * d.stair_step) {
            offsets.push(Vec3::new(0.0, y, 0.0));
            for k in [0.1f32, 0.25, 0.5, 0.75, 1.0] {
                offsets.extend(dirs.iter().map(|&(x, z)| Vec3::new(x * k * r, y, z * k * r)));
            }
        }
        offsets.sort_by(|a, b| a.length_squared().total_cmp(&b.length_squared()));
        if let Some(o) = offsets.into_iter().find(|o| space.fits(pos + *o, me.half_height)) {
            pos += o;
        }
    }
    if on_ground
        && let Some(hit) = space.cast(pos, me.half_height, Vec3::NEG_Y * d.stair_step)
        && hit.normal.y >= d.walkable_normal
    {
        pos.y -= hit.distance;
    }
    me.fall_top = Some(pos.y);
    pos
}

/// The spec's crouch curve: the Hermite-like shape with tangents `t0`, `t1`.
pub fn crouch_shape(t: f32, [t0, t1]: [f32; 2]) -> f32 {
    (3.0 * t * t - 4.0 * t + 1.0) * t0 + (-6.0 * t * t + 6.0 * t) + (3.0 * t * t - 2.0 * t) * t1
}

/// The fall damage (HP) of a drop of `d` u, if any (spec "Falling and fall
/// damage").
pub fn fall_damage(d: f32, data: &MovementData) -> Option<f32> {
    (d > data.fall_minimum && d >= data.fall_damage_start).then(|| {
        data.fall_damage_at_full * (d - data.fall_damage_start) / (data.fall_damage_full - data.fall_damage_start)
    })
}

/// The spec's fixed movement clock (spec "Tick clock"): real time in, whole
/// ticks out, at most `max_per_frame` a frame (the rest dropped), and the
/// leftover fraction for drawing between ticks. Not the engine's clock
/// (see the module's known shortcuts).
#[derive(Clone, Copy, Debug)]
pub struct TickClock {
    pub tick_ms: u32,
    pub max_per_frame: u32,
    accumulator_ms: f64,
}

impl TickClock {
    /// The spec's: 15 ms ticks, at most `floor(100 ms / tick)` a frame.
    pub fn new(tick_ms: u32) -> Self {
        Self {
            tick_ms,
            max_per_frame: 100 / tick_ms,
            accumulator_ms: 0.0,
        }
    }

    pub fn with_remainder(mut self, ms: f64) -> Self {
        self.accumulator_ms = ms;
        self
    }

    /// Add a frame of `ms`: the ticks to run now.
    pub fn frame(&mut self, ms: f64) -> u32 {
        self.accumulator_ms += ms;
        let tick = self.tick_ms as f64;
        let due = (self.accumulator_ms / tick).floor() as u32;
        let run = due.min(self.max_per_frame);
        // Ticks beyond the cap are dropped with their time.
        self.accumulator_ms -= due as f64 * tick;
        run
    }

    /// The leftover fraction of a tick (interpolation factor).
    pub fn fraction(&self) -> f64 {
        self.accumulator_ms / self.tick_ms as f64
    }
}

/// The footing ray's reach below the hull centre (u): a fall starts when
/// no floor lies this close (view_models.md 6; characters.md Corrections 3).
pub const FOOTING_RAY: f32 = 150.0;

/// The world as the movement sees it, in world units.
struct Space<'a, 'w, 's> {
    mover: &'a MoveAndSlide<'w, 's>,
    filter: SpatialQueryFilter,
    scale: f32,
    skin: f32,
    radius: f32,
    is_character: &'a dyn Fn(Entity) -> bool,
    /// Not a step (spec "Slopes and steps"): characters and moving
    /// objects (anything but a static body).
    not_a_step: &'a dyn Fn(Entity) -> bool,
}

struct Hit {
    /// Safe distance (u) along the cast.
    distance: f32,
    /// The contact on the surface hit (u).
    point: Vec3,
    normal: Vec3,
    entity: Entity,
}

impl Space<'_, '_, '_> {
    fn shape(&self, half_height: f32) -> Collider {
        Collider::cylinder(self.radius * self.scale, 2.0 * half_height * self.scale)
    }

    /// Sweep the hull from `centre` by `delta`: the first thing it would
    /// touch, if any. The shape cast stops short of flat faces by up to a
    /// few units on long sweeps (GJK's tolerance), so a hit is swept again
    /// from where it stopped until it stops moving; a hit that a closer
    /// sweep no longer finds was not there.
    fn cast(&self, centre: Vec3, half_height: f32, delta: Vec3) -> Option<Hit> {
        let len = delta.length();
        if len < 1e-6 {
            return None;
        }
        let dir = delta / len;
        let mut travelled = 0.0;
        let mut last = None;
        for _ in 0..6 {
            let rest = len - travelled;
            if rest <= 1e-4 {
                break;
            }
            let hit = self.cast_once(centre + dir * travelled, half_height, dir * rest)?;
            travelled += hit.distance.min(rest);
            let done = hit.distance < 1e-3;
            last = Some(hit);
            if done {
                break;
            }
        }
        last.map(|h| Hit { distance: travelled, ..h })
    }

    fn cast_once(&self, centre: Vec3, half_height: f32, delta: Vec3) -> Option<Hit> {
        let hit = self.mover.cast_move(
            &self.shape(half_height),
            centre * self.scale,
            Quat::IDENTITY,
            delta * self.scale,
            self.skin * self.scale,
            &self.filter,
        )?;
        Some(Hit {
            distance: hit.distance / self.scale,
            point: hit.point1 / self.scale,
            normal: self.face_normal(hit.point1, hit.normal1, hit.entity),
            entity: hit.entity,
        })
    }

    /// The surface's own normal at a cast's contact (meters): a short ray
    /// into the surface there. The shape cast's normal for a cylinder is
    /// off by up to a few hundredths, enough to tilt "flat" ground.
    fn face_normal(&self, point: Vec3, approx: Vec3, entity: Entity) -> Vec3 {
        let Ok(dir) = Dir3::new(-approx) else {
            return approx;
        };
        let back = 0.5 * self.scale;
        self.mover
            .spatial_query
            .cast_ray(point + approx * back, dir, 2.0 * back, true, &self.filter)
            .filter(|h| h.entity == entity && h.normal.dot(approx) > 0.5)
            .map_or(approx, |h| h.normal)
    }

    fn fits(&self, centre: Vec3, half_height: f32) -> bool {
        self.mover
            .spatial_query
            .shape_intersections(&self.shape(half_height), centre * self.scale, Quat::IDENTITY, &self.filter)
            .is_empty()
    }

    /// Push the hull out of anything it overlaps.
    fn depenetrate(&self, centre: Vec3, half_height: f32) -> Vec3 {
        if self.fits(centre, half_height) {
            return centre;
        }
        let config = DepenetrationConfig {
            skin_width: self.skin * self.scale,
            ..default()
        };
        let offset = self
            .mover
            .depenetrate(&self.shape(half_height), centre * self.scale, Quat::IDENTITY, &config, &self.filter);
        centre + offset / self.scale
    }

    /// Slide `delta` from `centre` (spec "Slopes and steps", "Sliding"):
    /// along the first surface hit (`deflect`), along the crease of the
    /// first two that aren't parallel, and stop at a third contact or a
    /// second parallel one. The end and what was hit (normal, entity).
    fn slide(&self, mut centre: Vec3, half_height: f32, delta: Vec3) -> (Vec3, Vec<(Vec3, Entity)>) {
        let mut normals: Vec<(Vec3, Entity)> = Vec::new();
        let mut o = delta;
        for _ in 0..5 {
            let len = o.length();
            if len < 1e-6 || (!normals.is_empty() && (len < 0.01 || o.dot(delta) <= 0.0)) {
                break;
            }
            let Some(hit) = self.cast(centre, half_height, o) else {
                centre += o;
                break;
            };
            let t = hit.distance.min(len);
            centre += o * (t / len);
            let rest = o * ((len - t) / len);
            let n = hit.normal;
            match normals.first() {
                None => o = deflect(rest, n),
                Some(&(n1, _)) if normals.len() == 1 && n.dot(n1).abs() < 0.999 => {
                    let c = n.cross(n1).normalize_or_zero();
                    o = c * rest.dot(c);
                }
                // A second contact parallel to the first, or a third.
                Some(_) => {
                    normals.push((n, hit.entity));
                    break;
                }
            }
            normals.push((n, hit.entity));
        }
        (centre, normals)
    }

    /// What the hull at `centre` stands on, within `probe` u below
    /// (spec "Slopes and steps", "What the player stands on"); `before`:
    /// what it stood on last.
    fn ground(
        &self,
        data: &MovementData,
        centre: Vec3,
        half_height: f32,
        probe: f32,
        before: Option<Standing>,
    ) -> Option<Standing> {
        let len = probe + self.skin;
        let hit = self.cast(centre, half_height, Vec3::NEG_Y * len)?;
        if hit.normal.y < 0.001 {
            return None;
        }
        let character = (self.is_character)(hit.entity);
        let steep = hit.normal.y < data.walkable_normal;
        if steep
            && let Some(old) = before.filter(|b| !b.character && b.normal.y >= data.walkable_normal)
        {
            // A slope beside the ground it stood on: it keeps standing on
            // that ground unless a probe from a step up, moved up-hill,
            // meets something.
            let ny = hit.normal.y;
            let t = (hit.distance / len).clamp(0.0, 1.0);
            let step = data.stair_step;
            let ahead = (0.5 * self.radius)
                .max(2.0 * (step + data.standing_probe * half_height * t) * (1.0 - ny * ny) + 0.11 * self.radius);
            let uphill = -Vec3::new(hit.normal.x, 0.0, hit.normal.z).normalize_or_zero();
            let from = centre + Vec3::Y * step;
            if uphill != Vec3::ZERO && self.cast(from, half_height, uphill * ahead).is_none() {
                return Some(old);
            }
        }
        Some(Standing {
            normal: hit.normal,
            character,
        })
    }

    /// Grow the hull from `old` to `new` half-height (spec "Crouching",
    /// "Growing"): down by up to the growth, then up by up to twice it,
    /// centred in the span found. Returns the centre and whether it fits
    /// (when not, it is forced around the old centre).
    fn grow(&self, centre: Vec3, old: f32, new: f32) -> (Vec3, bool) {
        let d = new - old;
        let down = self.cast(centre, old, Vec3::NEG_Y * d).map_or(d, |h| h.distance.min(d));
        let low = centre - Vec3::Y * down;
        let up = self.cast(low, old, Vec3::Y * (2.0 * d)).map_or(2.0 * d, |h| h.distance.min(2.0 * d));
        if old + up / 2.0 < new - 1e-4 {
            return (centre, false);
        }
        (low + Vec3::Y * (up / 2.0), true)
    }
}

/// The slide off a surface of normal `n` (spec "Slopes and steps",
/// "Sliding"): a move `o` into it loses its part into the surface and
/// 0.1 % more (pushed very slightly off it).
pub fn deflect(o: Vec3, n: Vec3) -> Vec3 {
    let into = o.dot(n);
    if into < 0.0 { o - n * (1.001 * into) } else { o }
}

/// A move `delta` from a floor of normal `n` (spec "Slopes and steps",
/// step 1): on a steep floor (`0 < n.y < walkable`) facing against the
/// move, a move whose part along the floor's plane goes up is replaced,
/// vertical part included, by its part along the slope's level direction.
pub fn steep_cut(n: Vec3, delta: Vec3, walkable: f32) -> Vec3 {
    if delta.dot(n) > 0.0 || n.y <= 0.0 || n.y >= walkable || (delta - n * delta.dot(n)).y <= 0.0 {
        return delta;
    }
    let level = n.cross(Vec3::Y).normalize_or_zero();
    level * delta.dot(level)
}

/// Braking (spec "Stopping"): the horizontal speed loses `dt x k / 0.1`;
/// reaching or passing 0 it becomes 0, the vertical speed kept; at 5 u/s
/// or less the whole velocity is 0.
pub fn brake(v: Vec3, k: f32, dt: f32, data: &MovementData) -> Vec3 {
    let h = Vec2::new(v.x, v.z);
    let speed = h.length();
    if speed <= data.stop_snap_speed {
        return Vec3::ZERO;
    }
    let slower = speed - dt * k / data.slide_to_stop_time;
    let h = if slower <= 0.0 { Vec2::ZERO } else { h * (slower / speed) };
    Vec3::new(h.x, v.y, h.y)
}

/// Spec "Slopes and steps": the collision move with stair stepping, from
/// a floor of normal `floor` when standing on something.
fn move_hull(space: &Space, data: &MovementData, centre: Vec3, half_height: f32, delta: Vec3, floor: Option<Vec3>) -> Vec3 {
    // 1. Up a steep floor: only its level part.
    let delta = floor.map_or(delta, |n| steep_cut(n, delta, data.walkable_normal));
    // 2. The low path, kept when it touched nothing, or anything that is
    // not a step.
    let (low, hits) = space.slide(centre, half_height, delta);
    let horizontal = Vec2::new(delta.x, delta.z);
    if horizontal.length_squared() < 1e-10 || hits.is_empty() || hits.iter().any(|(_, e)| (space.not_a_step)(*e)) {
        return low;
    }
    // 3. Lift (a low ceiling: keep the low result).
    let step = data.stair_step;
    if space.cast(centre, half_height, Vec3::Y * step).is_some() {
        return low;
    }
    // 4. The high path: slide from the lifted start, drop back at most the
    // lift.
    let lifted = centre + Vec3::Y * step;
    let (across, high_hits) = space.slide(lifted, half_height, delta);
    let high = match space.cast(across, half_height, Vec3::NEG_Y * step) {
        None => across - Vec3::Y * step,
        Some(hit) => {
            let landed = across - Vec3::Y * hit.distance;
            // What the flat bottom lands on carries its normal; a contact
            // higher up is an edge touching the curved side, whose normal
            // is level: steep, with no up-hill direction.
            let side = hit.point.y > landed.y - half_height + 2.0 * space.skin;
            let n = if side { Vec3::new(hit.normal.x, 0.0, hit.normal.z) } else { hit.normal };
            // 5. A steep top: refused when the high slide hit anything,
            // else unless a probe further up-hill is not steep.
            if n.y < data.walkable_normal {
                let uphill = -Vec3::new(n.x, 0.0, n.z).normalize_or_zero();
                if !high_hits.is_empty() || n.y < 0.001 || uphill == Vec3::ZERO {
                    return low;
                }
                let ny = n.y;
                let ahead = (0.5 * space.radius).max(2.0 * step * (1.0 - ny * ny) + 0.1);
                let target = across + uphill * ahead - lifted;
                let probe = lifted + space.cast(lifted, half_height, target).map_or(target, |h| target.normalize() * h.distance);
                let ok = space
                    .cast(probe, half_height, Vec3::NEG_Y * step)
                    .is_none_or(|h| h.normal.y >= data.walkable_normal);
                if !ok {
                    return low;
                }
            }
            landed
        }
    };
    // 6. Take the high result when it got further along the move.
    let o = Vec2::new(delta.x, delta.z);
    let along = |p: Vec3| Vec2::new(p.x - centre.x, p.z - centre.z).dot(o);
    let (l, h) = (along(low).max(0.0), along(high));
    let same_xz = Vec2::new(low.x - high.x, low.z - high.z).length() < 1e-4;
    if same_xz {
        if high.y >= low.y { high } else { low }
    } else if 1.01 * l < h {
        high
    } else {
        low
    }
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn step(
    mut q: Query<(
        Entity,
        &Intent,
        &mut CaMovement,
        &mut Transform,
        &mut Velocity,
        &mut MovementState,
        Option<&CaCharacter>,
        (
            Option<&HeldMovePenalty>,
            Option<&StunPenalty>,
            Option<&ModeSpeed>,
            Option<&CaAction>,
            Option<&CustomMovePenalty>,
            Option<&VipMovement>,
            Option<&super::perks::CaPerkEffects>,
        ),
        Option<&mut HitKnockback>,
        Has<Zoomed>,
        Has<Dead>,
        Has<MovementLocked>,
    )>,
    characters: Query<(), With<Intent>>,
    bodies: Query<&RigidBody>,
    collider_of: Query<&ColliderOf>,
    move_and_slide: MoveAndSlide,
    data: Res<MovementData>,
    clock: Res<SimClock>,
    first: Option<Res<FirstTimePredicted>>,
    mut damage: MessageWriter<Damage>,
    mut launches: MessageWriter<super::volumes::PadLaunch>,
    volumes: Query<&CaVolume>,
    pushers: Query<(Entity, &CaPusher)>,
    mut commands: Commands,
    ai: Query<(), Or<(With<crate::core::Bot>, With<crate::core::Npc>)>>,
) {
    let dt = clock.dt();
    let d = &*data;
    let s = d.meters_per_unit;
    let now = clock.now;
    let first = first.is_none_or(|f| f.0);
    let is_character = |e: Entity| characters.contains(e);
    // Characters and moving objects are never steps.
    let not_a_step = |e: Entity| {
        let body = collider_of.get(e).map_or(e, |c| c.body);
        characters.contains(e) || characters.contains(body) || bodies.get(body).is_ok_and(|b| !b.is_static())
    };
    // The pushers' clocks, once a tick for everyone.
    let mut active = Vec::new();
    for (e, p) in &pushers {
        match p.phase(now, dt) {
            PusherPhase::Active => active.push(*p),
            PusherPhase::Over if first => commands.entity(e).despawn(),
            _ => {}
        }
    }
    // The open-form step: no friction, a quarter-step displacement.
    let quarter = |v: Vec3, a: Vec3| (v + a * dt, v * dt + a * (0.25 * dt * dt));
    for (entity, intent, mut me, mut transform, mut vel, mut state, character, (held, stun, mode, action, custom, vip, perk), mut knock, zoomed, dead, lock) in
        &mut q
    {
        let me = &mut *me;
        let character = character.unwrap_or(&d.character);
        let mode = mode.copied().unwrap_or_default();
        let locked = !dead && (lock || action.is_some_and(|a| a.kind == ActionKind::Plant));
        me.locked = locked;
        // A VIP: the figure without its gear's bonuses.
        let vip_character;
        let character = match vip {
            Some(_) => {
                vip_character = character.vip();
                &vip_character
            }
            None => character,
        };
        let space = Space {
            mover: &move_and_slide,
            // The map's collision blockers stop players only (world_dat.md
            // Q5): bots and mercenaries pass them.
            filter: SpatialQueryFilter::from_excluded_entities([entity]).with_mask(if ai.contains(entity) {
                CHARACTER_FILTER.0 & !crate::core::PLAYER_CLIP_LAYER.0
            } else {
                CHARACTER_FILTER.0
            }),
            scale: s,
            skin: d.skin,
            radius: d.stand_half_extents[0].max(d.stand_half_extents[2]),
            is_character: &is_character,
            not_a_step: &not_a_step,
        };
        let stand_h = d.stand_half_extents[1];
        let crouch_h = d.crouch_half_extents[1];
        if !me.initialized {
            me.initialized = true;
            if me.half_height <= 0.0 {
                me.half_height = stand_h;
            }
            me.jump_factor = 1.0;
            me.ramp_forward = d.start_accel;
            me.ramp_strafe = d.start_accel;
            me.direction_speed = d.run_forward;
            me.stamina = (character.stamina_max() * mode.stamina).floor();
            me.slowdown = 1.0;
            me.lockout = 1.0;
            me.last_gravity = d.gravity;
            me.friction_k = d.friction;
        }
        let mut pos = transform.translation / s;
        if let Some(on_ground) = me.seat.take() {
            pos = reseat(&space, d, me, pos, on_ground);
        }
        pos = space.depenetrate(pos, me.half_height);
        // Spawn and respawn (spec "Falling and fall damage"): the server
        // puts the player on the floor below the spawn point, and the fall
        // is measured from there.
        // A liquid's surface on the way down is that floor (ours: over a
        // sea the drop would end on the world's outer box).
        if me.fall_top.is_none() || (me.was_dead && !dead) {
            let drop = space.cast(pos, me.half_height, Vec3::NEG_Y * SPAWN_DROP).map(|h| h.distance);
            let feet = pos.y - me.half_height;
            let lowest = feet - drop.unwrap_or(SPAWN_DROP);
            let at = pos * s;
            let liquid = volumes
                .iter()
                .filter(|v| v.liquid && !v.hidden)
                .filter(|v| at.x >= v.min.x && at.x <= v.max.x && at.z >= v.min.z && at.z <= v.max.z)
                .map(|v| v.max.y / s)
                .filter(|&top| top <= feet + 1e-3 && top > lowest)
                .fold(None, |a: Option<f32>, t| Some(a.map_or(t, |a| a.max(t))));
            match (liquid, drop) {
                (Some(top), _) => pos.y = top + me.half_height,
                (None, Some(d)) => pos.y -= d,
                (None, None) => {}
            }
            me.fall_top = Some(pos.y);
            // Entering the world clears a pending jump.
            me.jump_pending = false;
        }
        me.was_dead = dead;
        // Where it was put since (a teleport) counts for the fall too.
        if me.standing.is_none() {
            me.fall_top = me.fall_top.map(|t| t.max(pos.y));
        }
        let mut v = vel.0 / s;
        let spam = &d.crouch_spam;
        let c0 = 1.0 / spam.duration;

        // 2. The eye volume (the camera point: last tick's centre and
        // crouch rate) and the volumes the hull box touches (spec
        // "Volumes", "Membership"). Safety nets are markers only: they
        // count for falls, by the centre.
        let eye = (pos + Vec3::Y * eye_above_centre(me.crouch_rate, d)) * s;
        let eye_volume = super::volumes::eye_volume(volumes.iter(), eye);
        let eye_free = !dead && eye_volume.is_some_and(|v| v.liquid || v.is_ladder());
        let box_half = Vec3::new(space.radius, me.half_height, space.radius);
        let touched: Vec<&CaVolume> = volumes
            .iter()
            .filter(|v| v.touches_box((pos - box_half) * s, (pos + box_half) * s))
            .collect();
        let visible: Vec<&CaVolume> = touched.iter().copied().filter(|v| !v.hidden && !v.is_safety_net()).collect();
        let ladder = visible.iter().copied().find(|v| v.is_ladder());
        let on_ladder = !dead && ladder.is_some();
        let free = eye_free || on_ladder;
        let was_on_ladder = me.on_ladder;
        let was_eye_free = me.eye_free;
        me.on_ladder = on_ladder;
        me.in_liquid = eye_volume.is_some_and(|v| v.liquid);
        me.eye_free = eye_free;
        me.free_movement = free;

        // 3. Control flags (spec "Input to control state"). While locked
        // the movement keys set nothing; crouch and jump still work.
        let was = (me.forward, me.back, me.left, me.right, me.moving, me.jump);
        let keys_on = !dead;
        let move_keys = keys_on && !locked;
        let r_prev = me.crouch_rate;
        // 1. Crouch: the key, held on by the release delay after a press
        // and, fully crouched, by a lack of headroom; a press is this flag
        // turning on. Then the lock-out clears it.
        let held_flag = if keys_on && intent.crouch {
            true
        } else {
            let delay = me.crouch_held && now - me.crouch_pressed_at < d.crouch_release_delay - 1e-9;
            let fully = me.half_height <= crouch_h + 1e-4;
            let feet = pos.y - me.half_height;
            let no_room = me.crouch_held && fully && !space.fits(Vec3::new(pos.x, feet + stand_h, pos.z), stand_h);
            delay || no_room
        };
        let key_press = held_flag && !me.crouch_held;
        if key_press {
            me.crouch_pressed_at = now;
        }
        me.crouch_held = held_flag;
        me.crouch = held_flag && !(spam.enable_lockout && me.crouch_locked && r_prev == 0.0);
        // 2. Run: walk key XOR always-run (on); set at rest too.
        me.run = !me.crouch && keys_on && !intent.walk;
        // 3. Jump.
        me.jump = false;
        if !me.crouch && me.on_ground && !on_ladder && !me.in_jump {
            if keys_on && intent.jump {
                if !me.jump_used {
                    me.jump_used = true;
                    me.jump = me.half_height >= stand_h - d.jump_stance_tolerance;
                }
            } else {
                me.jump_used = false;
            }
        }
        // 4. Crouch rate not 0: no jump, no sprint.
        let mut sprint_ok = true;
        if r_prev != 0.0 {
            me.jump = false;
            sprint_ok = false;
        }
        // 5. Forward and sprint. Opposing keys both set their flags.
        let axis = intent.move_axis;
        me.forward = move_keys && (axis.y > 0.0 || intent.opposed.y);
        me.back = move_keys && (axis.y < 0.0 || intent.opposed.y);
        me.left = move_keys && (axis.x < 0.0 || intent.opposed.x);
        me.right = move_keys && (axis.x > 0.0 || intent.opposed.x);
        // The hit knock-back: no run or sprint while it runs.
        let knocked = knock.as_mut().and_then(|k| {
            if k.started.is_none() {
                k.started = Some(now);
            }
            k.running(now)
        });
        if knocked.is_some() {
            me.run = false;
        }
        let can_sprint = me.run && !on_ladder && r_prev == 0.0 && sprint_ok;
        // Victory Lap (gear_perks.md 14): the sprint key does nothing.
        let sprint_key = intent.sprint && !perk.is_some_and(|p| p.victory_lap);
        if me.forward {
            if sprint_key && me.stamina.floor() >= 1.0 && can_sprint {
                me.sprint = true;
                me.stamina_state = StaminaState::Draining;
            } else if sprint_key && can_sprint {
                me.sprint = false;
                me.stamina_state = StaminaState::Stopped;
            } else {
                me.sprint = false;
                me.stamina_state = StaminaState::Recovering;
            }
        } else {
            me.sprint = false;
            me.stamina_state = StaminaState::Recovering;
        }
        // 6. Back ends the sprint, leaving the gauge's state (forward,
        // back and sprint drain it: Quirks).
        if me.back || knocked.is_some() {
            me.sprint = false;
        }
        // 7. Moving.
        me.moving = me.forward || me.back || me.left || me.right || me.jump;
        // 8. A jump request, remembered until it can happen.
        if me.jump && !was.5 && !me.in_jump {
            me.jump_pending = true;
        }

        // 4. Crouch rate, then the crouch-spam values.
        let c = if spam.enable_lockout && me.crouch_locked {
            0.0
        } else if spam.enable_slowdown && spam.min_slowdown_sp.abs() > 1e-6 && me.slowdown <= spam.min_slowdown_sp {
            let k = me.slowdown.clamp(0.0, spam.min_slowdown_sp) / spam.min_slowdown_sp;
            c0 / (spam.max_slowdown_rate + (1.0 - spam.max_slowdown_rate) * k)
        } else {
            c0
        };
        if dead || free {
            // Dead, on a ladder or in free movement: reset.
            me.crouch_rate = 0.0;
            me.crouch_phase = CrouchPhase::Idle;
            me.crouch_elapsed = 0.0;
        } else if me.crouch && c > 0.0 {
            if me.crouch_phase != CrouchPhase::Down {
                me.crouch_phase = CrouchPhase::Down;
                me.crouch_elapsed = 0.0;
            }
            let t = me.crouch_elapsed * c;
            me.crouch_rate += crouch_shape(t, spam.tangents) * dt * c;
            me.crouch_elapsed += dt;
            if me.crouch_rate >= 1.0 - 1e-6 {
                me.crouch_rate = 1.0;
            }
        } else if me.crouch_rate > 0.0 {
            if me.crouch_phase != CrouchPhase::Up {
                me.crouch_phase = CrouchPhase::Up;
                me.crouch_elapsed = 0.0;
            }
            let c_up = if c == 0.0 { c0 } else { c };
            let t = me.crouch_elapsed * c_up;
            me.crouch_rate -= crouch_shape(t, spam.tangents) * dt * c_up;
            me.crouch_elapsed += dt;
            if me.crouch_rate <= 1e-6 {
                me.crouch_rate = 0.0;
            }
        } else {
            me.crouch_phase = CrouchPhase::Idle;
            me.crouch_elapsed = 0.0;
        }
        let r = me.crouch_rate;
        crouch_spam(me, spam, r - r_prev, key_press, dt);

        // 5. Resize the hull (once: the spec's second resize changes
        // nothing). Shrinking keeps the feet when standing, else the
        // centre; growing finds its room below and above.
        let new_h = stand_h - r * (stand_h - crouch_h);
        if new_h < me.half_height {
            if me.standing.is_some() {
                pos.y -= me.half_height - new_h;
            }
            me.half_height = new_h;
        } else if new_h > me.half_height {
            // Not fitting: forced around the old centre.
            pos = space.grow(pos, me.half_height, new_h).0;
            me.half_height = new_h;
        }

        // 6. The last direction speed.
        if me.forward {
            me.direction_speed = d.run_forward;
        } else if me.back {
            me.direction_speed = d.run_back;
        } else if me.left || me.right {
            me.direction_speed = d.run_strafe;
        }
        let k = me.direction_speed;
        let fig = &character.figure;

        // 7. The motion step.
        let mut a;
        let gravity_off;
        let gravity;
        let friction_k;
        if locked {
            // 7a. Movement disallowed: last tick's acceleration keeps its
            // downward part (at most gravity's), the horizontal speed is
            // halved and an upward one removed; nothing else of the step.
            a = Vec3::new(0.0, me.applied_accel.y.clamp(d.gravity, 0.0), 0.0);
            v.x *= 0.5;
            v.z *= 0.5;
            v.y = v.y.min(0.0);
            gravity_off = was_on_ladder;
            gravity = if gravity_off { 0.0 } else { me.last_gravity };
            friction_k = me.friction_k;
            me.accel = a;
        } else {
            // 7b. Volumes: gravity on unless on a ladder last tick; the
            // first ladder's viscosity and speed factor, then every other
            // visible volume's viscosity (not in a jump, except on a
            // ladder), gravity and speed factor; every one's current.
            gravity_off = on_ladder || was_on_ladder;
            let skip_viscosity = me.in_jump && !on_ladder;
            let mut viscosity = 0.0;
            let mut current = Vec3::ZERO;
            let mut factor = 1.0;
            let mut pull: Option<f32> = None;
            let thick = |vol: &CaVolume, v: &mut Vec3, viscosity: &mut f32| {
                if vol.viscosity > 0.0 && !skip_viscosity {
                    *viscosity += vol.viscosity;
                    let speed = v.length();
                    if speed > d.viscosity_min_speed {
                        let slower = speed - d.viscosity_max * vol.viscosity * dt;
                        *v = if slower > 0.0 { *v * (slower / speed) } else { Vec3::ZERO };
                    }
                }
            };
            if let Some(l) = ladder {
                thick(l, &mut v, &mut viscosity);
                factor *= l.speed_factor;
            }
            for vol in &visible {
                current += vol.current;
                if vol.is_ladder() {
                    continue;
                }
                thick(vol, &mut v, &mut viscosity);
                factor *= vol.speed_factor;
                if let Some(g) = vol.player_gravity {
                    pull = Some(pull.unwrap_or(0.0) + g);
                }
            }
            me.last_gravity = pull.unwrap_or(d.gravity);
            gravity = if gravity_off { 0.0 } else { me.last_gravity };
            // Ground friction: 5 x the least `Friction` touched, hidden
            // volumes too.
            friction_k = d.friction
                * touched
                    .iter()
                    .filter(|v| v.sets_friction())
                    .map(|v| v.friction.clamp(0.0, 1.0))
                    .fold(1.0, f32::min);
            me.friction_k = friction_k;
            me.brake_blocked = current * dt != Vec3::ZERO || viscosity != 0.0;
            // A jump pad (switched off too) requests a jump when the
            // player stood on ground last tick; not in free movement.
            let pad_volume = touched.iter().filter(|v| v.jump_velocity.is_some()).last();
            let pad = pad_volume.and_then(|v| v.jump_velocity);
            if pad.is_some() && me.on_ground && !free {
                me.jump_pending = true;
            }

            // 7c. Ground from last tick's move; landing.
            if me.standing.is_some() && me.in_jump {
                me.in_jump = false;
            }
            me.on_ground = !me.in_jump
                && me.standing.is_some_and(|g| !g.character && g.normal.y >= d.on_ground_normal);
            me.no_footing = !me.on_ground
                && !me.in_jump
                && !free
                && !dead
                && space
                    .mover
                    .spatial_query
                    .cast_ray(transform.translation, Dir3::NEG_Y, FOOTING_RAY * s, true, &space.filter)
                    .is_none();

            // 7d. Speed limit and clamp (on a ladder only the upward speed
            // is capped); leaving free movement by the eye stops the
            // player.
            let crouching = me.crouch || r == 1.0;
            let victory_lap = perk.is_some_and(|p| p.victory_lap);
            // Victory Lap (gear_perks.md 14): the run flag uses the sprint
            // limit, the sprint flag and the gauge untouched.
            let v0 = if (me.sprint || (victory_lap && me.run)) && !crouching && !zoomed {
                k * fig.dash_speed * (1.0 + character.gear_sum(|g| g.dash_speed))
            } else if me.run && !crouching && !zoomed {
                k
            } else {
                d.walk
            };
            let v0 = v0.max(d.crouch);
            let v1 = if on_ladder { d.ladder } else { v0 + (d.crouch - v0) * r };
            let held = held.copied().unwrap_or_default();
            let stun = stun.filter(|s| now < s.until).map_or(0.0, |s| s.penalty);
            let custom = custom.map_or(0.0, |c| c.0);
            let penalty = match vip {
                Some(v) => v.penalty.clamp(-1.0, 1.0),
                None => (held.penalty * (1.0 + held.mod_factor)
                    + character.gear_sum(|g| g.move_penalty)
                    + fig.variable_weight
                    - fig.variable_speed
                    + custom
                    + stun)
                    .clamp(-1.0, 1.0),
            };
            let limit = v1 * (1.0 - penalty) * mode.speed * factor * character.speed_scale;
            me.speed_limit = limit;
            if on_ladder {
                v.y = v.y.min(d.ladder);
            } else {
                let h = Vec2::new(v.x, v.z);
                if h.length() > limit {
                    let h = h.normalize_or_zero() * limit;
                    v.x = h.x;
                    v.z = h.y;
                }
            }
            if was_eye_free && !free {
                v = Vec3::ZERO;
            }

            // 7e. Acceleration. On a ladder forward is the camera's full
            // direction; swimming, the camera's without an upward part.
            let look = intent.look_rotation() * Vec3::NEG_Z;
            let yaw = intent.yaw_rotation();
            let fwd = if on_ladder {
                look
            } else if free {
                look.with_y(look.y.min(0.0)).normalize_or_zero()
            } else {
                (yaw * Vec3::NEG_Z).with_y(0.0).normalize_or_zero()
            };
            let right = (yaw * Vec3::X).with_y(0.0).normalize_or_zero();
            let mut base = d.base_accel;
            if me.crouch && !on_ladder {
                base *= d.crouch_accel_factor;
            }
            if me.in_jump && !free {
                base *= d.in_air_accel_factor;
            }
            let start = me.moving && !was.4;
            // A pending jump, a jump in free movement and crouching skip
            // the ramp: no start rescale, the remembered ramp values kept.
            let bypass = me.jump_pending || (free && me.jump) || me.crouch;
            let (a_f, a_r) = if bypass {
                (base, base)
            } else {
                let inc = d.accel_ramp * dt;
                if start {
                    me.ramp_forward = d.start_accel;
                    me.ramp_strafe = d.start_accel;
                }
                let ramp = |l: &mut f32| {
                    let n = *l + inc;
                    *l = if n < d.max_accel { n } else { d.max_accel };
                    *l
                };
                // A reversal: one key held now and the other last tick
                // (holding both is one on every tick).
                let a_f = if (me.forward && was.1) || (me.back && was.0) {
                    me.ramp_forward = d.start_accel;
                    d.start_accel
                } else if me.forward || me.back {
                    // The start tick: the base, unramped (the rescale
                    // below takes it to 500).
                    if start { base } else { ramp(&mut me.ramp_forward) }
                } else {
                    me.ramp_forward = d.start_accel;
                    0.0
                };
                let a_r = if (me.right && was.2) || (me.left && was.3) {
                    me.ramp_strafe = d.start_accel;
                    d.start_accel
                } else if me.left || me.right {
                    ramp(&mut me.ramp_strafe)
                } else {
                    me.ramp_strafe = d.start_accel;
                    0.0
                };
                (a_f, a_r)
            };
            a = Vec3::ZERO;
            if me.forward {
                a += fwd * a_f;
            }
            if me.back {
                a -= fwd * a_f;
            }
            if me.left {
                a = (a - right * a_r).clamp_length_max(d.max_accel);
            }
            if me.right {
                a = (a + right * a_r).clamp_length_max(d.max_accel);
            }
            // Crouched in free movement: down at the (unhalved on a
            // ladder) base, in place of the keys' vertical part.
            if free && me.crouch {
                a.y = -base;
            }
            if start && !bypass && a != Vec3::ZERO {
                a = a.normalize() * d.start_accel;
            }
            let j = me.jump_factor;
            if j < 1.0 {
                a *= j;
            }
            // The knock-back replaces it: K^2 (1 - t) along the hit's
            // horizontal direction; with the crouch flag, a quarter when
            // moving, none (and no velocity) when not.
            if let (Some(t), Some(kb)) = (knocked, knock.as_deref()) {
                let dir = kb.dir.with_y(0.0).normalize_or_zero();
                a = dir * kb.strength * kb.strength * (1.0 - t);
                if me.crouch {
                    if me.moving {
                        a *= 0.25;
                    } else {
                        a = Vec3::ZERO;
                        v = Vec3::new(0.0, v.y, 0.0);
                    }
                }
            }

            // 7f. A requested (or pending) jump when on ground; else the
            // jump factor's recovery (not while one is pending).
            if me.jump_pending && !dead && me.on_ground && !on_ladder {
                // A jump pad adds its own launch velocity instead.
                if let Some(pad) = pad {
                    v += pad;
                    // The pad's world sound, from the launched player
                    // (volumes, world_dat.md Corrections (2026-10-22) 7).
                    if first
                        && let Some(sound) = pad_volume.and_then(|p| p.pad_sound.clone())
                    {
                        launches.write(super::volumes::PadLaunch { at: pos * s, sound });
                    }
                } else {
                    let bonus = fig.variable_jump + character.gear_sum(|g| g.jump);
                    v.y = d.jump * (1.0 + bonus) * j;
                }
                // From the bottom of a liquid: a push up on this tick.
                if free {
                    a.y += d.base_accel * j.min(1.0);
                }
                me.jump_pending = false;
                me.in_jump = true;
                me.on_ground = false;
                me.jump_factor = if j < d.jump_factor_after_jump {
                    j - 0.05 * dt
                } else {
                    d.jump_factor_after_jump
                };
            } else if me.jump_pending {
            } else if j < 1.0 {
                me.jump_factor += d.jump_factor_recovery * dt;
            } else {
                me.jump_factor = 1.0;
            }
            // Free movement drops a request it didn't honour.
            if free {
                me.jump_pending = false;
            }
            // 7g. The volumes' currents; dying: no velocity, no acceleration.
            v += current * dt;
            if dead {
                v = Vec3::ZERO;
                a = Vec3::ZERO;
            }
            me.accel = a;
        }

        // 8. Braking with no movement keys (none under a knock-back, nor
        // while the volumes' currents or viscosities add up to anything).
        if !me.moving && me.on_ground && !me.in_jump && knocked.is_none() && !me.brake_blocked {
            v = brake(v, k, dt, d);
        }

        // 9. Pushers: the push, a tenth standing on nothing; then an
        // upward speed above the cap is cut to it.
        for push in &active {
            let from = push.origin / s;
            let line = pos - from;
            let dist = line.length();
            if push.radius <= 0.0 || dist >= push.radius {
                continue;
            }
            let clear = Dir3::new(line).map_or(true, |dir| {
                move_and_slide
                    .spatial_query
                    .cast_ray_predicate(push.origin, dir, dist * s, true, &space.filter, &|e| !not_a_step(e))
                    .is_none()
            });
            if !clear {
                continue;
            }
            let mut add = line.normalize_or_zero() * push.strength * (1.0 - dist / push.radius);
            if me.standing.is_none() {
                add *= d.pusher_air_factor;
            }
            v += add;
            if v.y > d.pusher_max_up {
                v.y = d.pusher_max_up;
            }
        }

        // 11. Integrate, move, stand, fix. Gravity off (a ladder): no
        // ground rules at all. Standing on a model (a character): a flat
        // normal.
        let g = Vec3::Y * gravity;
        if v.length_squared() < d.rest_threshold {
            v = Vec3::ZERO;
        }
        if a.length_squared() < d.rest_threshold {
            a = Vec3::ZERO;
        }
        let ground = me.standing.map(|g| if g.character { Vec3::Y } else { g.normal });
        let (v_new, dx) = if gravity_off {
            quarter(v, a)
        } else {
            match ground {
                Some(_) if v == Vec3::ZERO && a == Vec3::ZERO => (Vec3::ZERO, Vec3::ZERO),
                Some(n) if n.y < d.walkable_normal => {
                    let full = a + g;
                    let mut along = full - n * full.dot(n);
                    let down = (g - n * g.dot(n)).normalize_or_zero();
                    let up_slope = along.dot(down);
                    if up_slope < 0.0 {
                        along -= down * up_slope;
                    }
                    a = along.normalize_or_zero() * full.length();
                    quarter(v, a)
                }
                Some(n) => {
                    if (v + g * dt).dot(n) > 0.01 {
                        a += g;
                        quarter(v, a)
                    } else {
                        if a.dot(n) > 0.01 {
                            let len = a.length();
                            a = (a - n * (n.dot(a) + 1.0)).normalize_or_zero() * len;
                        }
                        let kf = friction_k;
                        if kf > 1e-6 {
                            let e = (-kf * dt).exp();
                            (a / kf + (v - a / kf) * e, a / kf * dt + (v * kf - a) * (1.0 - e) / (kf * kf))
                        } else {
                            // No friction (k = 0): the open form.
                            quarter(v, a)
                        }
                    }
                }
                None => {
                    a += g;
                    quarter(v, a)
                }
            }
        };
        me.applied_accel = a;
        v = v_new;
        let before = pos;
        if dx.length_squared() > 1e-6 {
            pos = move_hull(&space, d, pos, me.half_height, dx, ground);
        }
        let probe = d.standing_probe * me.half_height;
        me.standing = space.ground(d, pos, me.half_height, probe, me.standing);
        // Stuck against something while not on ground (the movement's
        // flag of this tick): no horizontal speed. Else a blocked vertical
        // move: no vertical speed. (Unchanged: within 0.01 u, the mover's
        // clearance jitter; the spec's 10^-6.)
        let moved = pos - before;
        if dx.x.abs() > 0.01 && dx.z.abs() > 0.01 && !me.on_ground && moved.length() < 0.01 {
            v.x = 0.0;
            v.z = 0.0;
        } else if dx.y.abs() > 0.01 && moved.y.abs() < 0.01 {
            v.y = 0.0;
        }

        // Fall damage (the server's, from positions): none on a ladder (the
        // highest point is reset there) or landing in a safety net.
        if on_ladder {
            me.fall_top = Some(f32::MIN);
        }
        let net = volumes.iter().any(|v| v.is_safety_net() && v.contains(pos * s));
        if me.standing.is_some() {
            if let Some(top) = me.fall_top {
                let drop = top - pos.y;
                if drop > 0.0
                    && let Some(hp) = fall_damage(drop, d)
                    && first
                    && !dead
                    && !on_ladder
                    && !net
                {
                    damage.write(Damage {
                        target: entity,
                        attacker: Some(entity),
                        amount: hp / d.health_points,
                        point: (pos - Vec3::Y * me.half_height) * s,
                        dir: Vec3::Y,
                        hitgroup: Hitgroup::Generic,
                        kind: DamageKind::Crush,
                        weapon: None,
                        force: Vec3::ZERO,
                    });
                }
            }
            me.fall_top = Some(pos.y);
        } else {
            me.fall_top = Some(me.fall_top.map_or(pos.y, |t| t.max(pos.y)));
        }

        // 13. Record for "still".
        me.history.push((now, Vec2::new(pos.x, pos.z)));
        let keep = now - spam.no_movement_time_limit as f64 - 0.1;
        me.history.retain(|(t, _)| *t >= keep);

        // Stamina: per tick here (the spec's client per frame), each step
        // at most the server's record clamp, so the authority's gauge
        // ends the sprint.
        let max = (character.stamina_max() * mode.stamina).floor();
        let step_s = dt.min(d.stamina_step_max);
        // The perks (gear_perks.md 14): War Cry and Virulent lose none, an
        // Awareness, Steady Hand or Inner Focus gauge doesn't recover, the
        // first two drain on their own.
        let (free, no_recover, extra) = perk.map_or((false, false, 0.0), |p| (p.free_stamina, p.no_recover, p.drain));
        match me.stamina_state {
            StaminaState::Draining if !free => me.stamina = (me.stamina - fig.dash_gauge * step_s).max(0.0),
            StaminaState::Draining => {}
            StaminaState::Recovering if !no_recover => me.stamina = (me.stamina + fig.gauge_recovery * step_s).min(max),
            StaminaState::Recovering => {}
            StaminaState::Stopped => me.stamina = 0.0,
        }
        if extra > 0.0 && !free {
            me.stamina = (me.stamina - extra * step_s).max(0.0);
        }

        transform.translation = pos * s;
        vel.0 = v * s;
        let r_xz = space.radius * s;
        let half = me.half_height * s;
        *state = MovementState {
            on_ground: !me.in_jump
                && me.standing.is_some_and(|g| !g.character && g.normal.y >= d.on_ground_normal),
            crouching: r > 0.0,
            sprinting: me.sprint,
            on_ladder,
            eye_offset: Vec3::Y * eye_above_centre(r, d) * s,
            hull_min: Vec3::new(-r_xz, -half, -r_xz),
            hull_max: Vec3::new(r_xz, half, r_xz),
            ..default()
        };
    }
}

/// Spec "Crouch-spam penalty": update the slowdown and lock-out values
/// after this tick's crouch rate change `dr`.
fn crouch_spam(me: &mut CaMovement, spam: &CrouchSpamData, dr: f32, key_press: bool, dt: f32) {
    let moving_change = dr.abs() > 1e-6;
    let crouch_start = moving_change && dr > 0.0 && !me.spam_rising;
    if moving_change {
        me.spam_rising = dr > 0.0;
    }
    if crouch_start {
        me.spam_elapsed = 0.0;
    } else {
        me.spam_elapsed += dt;
    }
    // "Still": horizontal distance over the last 0.2 s, up to last tick.
    let still = match me.history.last() {
        None => true,
        Some(&(newest, _)) => {
            let window: Vec<Vec2> = me
                .history
                .iter()
                .filter(|(t, _)| newest - *t <= spam.no_movement_time_limit as f64 + 1e-6)
                .map(|(_, p)| *p)
                .collect();
            let moved: f32 = window.windows(2).map(|w| (w[1] - w[0]).length()).sum();
            moved < spam.no_movement_deviation_limit
        }
    };
    let ground = me.on_ground;
    let charge = (!still || !spam.decrease_exception_no_movement) && (ground || spam.decrease_during_jumping_crouch);
    let charged = if spam.decrease_by_keystroke { key_press } else { crouch_start };

    if spam.enable_slowdown {
        if !crouch_start && !key_press && !moving_change {
            let t = me.spam_elapsed / spam.slowdown_increase_duration;
            me.slowdown += crouch_shape(t, spam.slowdown_increase_tangents) * dt / spam.slowdown_increase_duration;
        } else if charge && charged {
            me.slowdown -= spam.slowdown_decrease_amount;
        }
        me.slowdown = me.slowdown.clamp(0.0, 1.0);
    } else {
        me.slowdown = 1.0;
    }

    if spam.enable_lockout {
        let was_locked = me.crouch_locked;
        if !crouch_start && !key_press && ground {
            let t = me.spam_elapsed / spam.lockout_increase_duration;
            me.lockout += crouch_shape(t, spam.lockout_increase_tangents) * dt / spam.lockout_increase_duration;
            if me.lockout >= 1.0 {
                me.crouch_locked = false;
            }
        } else if charge && charged {
            me.lockout -= if was_locked {
                spam.lockout_decrease_amount_in_penalty
            } else {
                spam.lockout_decrease_amount
            };
            if me.lockout <= 0.0 {
                me.crouch_locked = true;
            }
        }
        me.lockout = me.lockout.clamp(0.0, 1.0);
    } else {
        me.lockout = 1.0;
        me.crouch_locked = false;
    }
}

/// The entity's collider follows the hull (an upright cylinder); a player
/// that leaves this movement gets the standard capsule back.
fn sync_collider(
    mut q: Query<(&CaMovement, &mut Collider), Changed<CaMovement>>,
    mut removed: RemovedComponents<CaMovement>,
    mut others: Query<&mut Collider, Without<CaMovement>>,
    data: Res<MovementData>,
) {
    let s = data.meters_per_unit;
    let radius = data.stand_half_extents[0].max(data.stand_half_extents[2]) * s;
    for e in removed.read() {
        if let Ok(mut c) = others.get_mut(e) {
            *c = Collider::capsule(CAPSULE_RADIUS, CAPSULE_HEIGHT - 2.0 * CAPSULE_RADIUS);
        }
    }
    for (me, mut collider) in &mut q {
        if me.half_height <= 0.0 {
            continue;
        }
        let half = me.half_height * s;
        let wanted = Collider::cylinder(radius, 2.0 * half);
        let same = collider
            .shape_scaled()
            .as_cylinder()
            .is_some_and(|c| (c.half_height - half).abs() < 1e-6 && (c.radius - radius).abs() < 1e-6);
        if !same {
            *collider = wanted;
        }
    }
}
