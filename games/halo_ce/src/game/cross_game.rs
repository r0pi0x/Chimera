//! Halo's balance where damage crosses games (the party apps:
//! data/party_balance.toml, `CrossGame`), the project owner's decision of
//! 2026-11-15: a Halo player a little more tanky than another game's, its
//! guns not also overpowered, Halo against Halo untouched, no shield left
//! as armour on the next rung. One switch (`enabled`) turns it all off,
//! today's behaviour exactly.
//!
//! - **Incoming** (another game's damage on a Halo player): it counts
//!   `incoming_scale` times against the shield and the body, the shield
//!   still first. The engine's armour step stays as it is for every game
//!   (`weapon::hits::absorb`): the shield's `weapon::Armor` holds the
//!   shield fraction divided by the scale (`shield_unit`: a full shield
//!   is 1 / 1.5 of another game's armour, so it breaks after 66.7 of its
//!   damage, which is 100 counted), and the body's `core::Health` takes
//!   another game's damage times the scale (`core::DamageScale` on the
//!   Halo player, applied by the engine's `apply_damage`; Halo's own hits
//!   are written divided by it, so they land as the spec says). Together
//!   that is the hit scaled before the armour step: a full Halo player has
//!   200 / 1.5 = 133 effective hit points against another game's gun.
//! - **Outgoing** (a Halo source on another game's character) is not here:
//!   Halo's conversion stays (a Halo point 1/75 of that character's
//!   health, `super::damage`), and the party's per-game damage table
//!   (`chimera_sim::weapon::GameDamage`, party_kit's
//!   data/party_damage.toml) scales it with every other game's, in one
//!   place for all.
//! - **The armour brought in** (`Brought`): a character becoming Halo's
//!   keeps today's rule (what armour it brings is the shield's start,
//!   docs/DESIGN.md "Armour when the identity switches"); one that stops
//!   being Halo's gets back what it brought (its value and kind, or none),
//!   never the shield's value (`super::vitality`).
//! - **The power reader's hand** (`PowerHand`): the taps a second a
//!   trigger with no rate limit fires at, and the bursts an automatic gun
//!   with growing error is fired in.

use bevy::prelude::*;
use chimera_sim::weapon::Armor;
use serde::Deserialize;

/// The data, as shipped.
pub const DATA: &str = include_str!("../../data/party_balance.toml");

/// What the power reader assumes of a hand (`[power]`).
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PowerHand {
    pub hand_taps_per_second: f32,
    pub burst_seconds: f32,
}

/// Halo's balance across games (data/party_balance.toml; the module doc).
#[derive(Resource, Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CrossGame {
    pub enabled: bool,
    pub incoming_scale: f32,
    pub power: PowerHand,
}

impl Default for CrossGame {
    fn default() -> Self {
        Self::load()
    }
}

impl CrossGame {
    /// The shipped data. Bad built-in data panics.
    pub fn load() -> Self {
        toml::from_str(DATA).unwrap_or_else(|e| panic!("halo_ce data/party_balance.toml: {e}"))
    }

    /// The shipped data with the switch off (today's behaviour).
    pub fn off() -> Self {
        Self { enabled: false, ..Self::load() }
    }

    /// What another game's damage counts against a Halo player (1 when
    /// off).
    pub fn incoming(&self) -> f32 {
        if self.enabled && self.incoming_scale.is_finite() && self.incoming_scale > 0.0 { self.incoming_scale } else { 1.0 }
    }

    /// Whether a character that stops being Halo's gets back the armour it
    /// brought (off: it keeps the shield's value).
    pub fn restores_armour(&self) -> bool {
        self.enabled
    }

    /// The world's, or the shipped data.
    pub fn of(world: &World) -> Self {
        world.get_resource::<CrossGame>().copied().unwrap_or_default()
    }
}

/// The shield's `Armor` amount for a shield fraction `shield` under the
/// incoming scale `scale` (`core::DamageScale`).
pub fn shield_unit(shield: f32, scale: f32) -> f32 {
    if scale == 1.0 { shield } else { shield / scale }
}

/// The shield fraction an `Armor` amount holds under `scale`.
pub fn shield_of(amount: f32, scale: f32) -> f32 {
    if scale == 1.0 { amount } else { amount * scale }
}

/// The armour a character had when it became Halo's (None: none): what
/// it gets back when it stops being Halo's (`CrossGame::restores_armour`).
/// Put on and taken off with the rest of Halo's parts, in the same move.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
#[component(storage = "SparseSet")]
pub struct Brought(pub Option<Armor>);
