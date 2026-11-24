//! What a game sets on a character before the step (`SourceMovementStep`): the
//! tick's max speed (`MoveSpeed`), its kind's traits (`MoveTraits`), its
//! own stance (`MoveStance`) and its footstep entries (`StepSounds`). The
//! names are the movement's own, whatever the game's engine; the Source
//! games' code says `SourceSpeed`, `SourceTraits` and `SourceStance`, the
//! same types under Source's names (`source_movement`'s re-exports).

use bevy::prelude::*;

use crate::config::SourceMovementConfig;

/// What a character's own kind changes in its movement (TF2's classes:
/// top speed, eye height, air jumps), set by its game before the step
/// (`SourceMovementStep`); none: the config as registered. `max_speed`
/// (units/s) is the tick's max speed as `MoveSpeed` is, before it: the
/// two are different games' (a game's own leaves the other's alone when a
/// player moves from one game's movement to the other's).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct MoveTraits {
    pub max_speed: Option<f32>,
    pub eye_stand: Option<f32>,
    pub air_jumps: Option<u8>,
    pub loud_landings_only: Option<bool>,
    /// The jump's and the air jump's speed × this (TF2's items: the
    /// Winger's 1.25 while out).
    pub jump_scale: Option<f32>,
    /// A speed ceiling above the config's (`maxspeed`), the keys' wish
    /// raised to it as well (TF2's shield charge: 750 past its 520).
    pub ceiling: Option<f32>,
}

impl MoveTraits {
    /// `cfg` with these traits.
    pub fn apply(&self, cfg: &SourceMovementConfig) -> SourceMovementConfig {
        let mut c = cfg.clone();
        if let Some(e) = self.eye_stand {
            c.eye_stand = e;
        }
        if let Some(n) = self.air_jumps {
            c.air_jumps = n;
        }
        if let Some(q) = self.loud_landings_only {
            c.loud_landings_only = q;
        }
        if let Some(s) = self.jump_scale {
            c.jump_impulse *= s;
            c.air_jump_speed *= s;
        }
        if let Some(m) = self.ceiling {
            c.maxspeed = c.maxspeed.max(m);
            c.key_speed = c.key_speed.max(m);
        }
        c
    }
}

/// This tick's max speed for a Source character, set by its game before
/// the movement (HL2:DM's walk, run and sprint, specs/hl2dm/movement.md
/// "Speed selection"), units/s: replaces the held weapon's and the walk
/// key's scale; `sv_maxspeed` still caps it.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[component(storage = "SparseSet")]
pub struct MoveSpeed(pub f32);

/// A game's own stance over standing and ducking (DoD:S's prone and its
/// transitions), set by the game before the step (`SourceMovementStep`)
/// each tick; the default changes nothing.
///
/// - `height`: the box is this high (units) while set: held as the ducked
///   box at that height (`SourceMovement::ducked`; the duck key does
///   nothing). When it goes, the character stands up at once where the
///   standing box fits, else stays ducked (the usual duck box) until it
///   does.
/// - `eye`: the eye over the feet this tick (units), over the stand or
///   duck eye (a game's own eased transition).
/// - `max_speed`: this tick's max speed (units/s), over the game's
///   (`MoveSpeed`), the traits' and the held weapon's; a hit's velocity
///   modifier still scales it.
/// - `no_jump`: the jump key does nothing.
/// - `crouch`: with `height`, leaving the stance lands ducked, as if duck
///   had been held all along: still held, the character stays ducked;
///   let go, it stands up by the usual unduck (DoD:S's prone to a crouch:
///   specs/dods/movement.md "Getting up").
/// - `no_input`: the move keys do nothing; the speed the character has
///   goes on, slowed by friction on the ground (DoD:S going prone,
///   movement.md "Prone": pressed while sprinting, the speed decays).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MoveStance {
    pub height: Option<f32>,
    pub eye: Option<f32>,
    pub max_speed: Option<f32>,
    pub no_jump: bool,
    #[serde(default)]
    pub crouch: bool,
    #[serde(default)]
    pub no_input: bool,
}

impl MoveStance {
    /// `cfg` with this stance's box as its ducked box.
    pub(crate) fn apply(&self, cfg: &mut SourceMovementConfig) {
        if let Some(h) = self.height {
            cfg.duck_height = h;
        }
    }
}

/// A character's own footstep entries (left, right), for a movement with
/// `model_footsteps` (HL2:DM: its model's run steps).
#[derive(Component, Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[component(storage = "SparseSet")]
pub struct StepSounds {
    pub left: String,
    pub right: String,
}
