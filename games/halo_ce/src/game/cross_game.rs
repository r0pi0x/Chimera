//! Halo's mechanisms where damage crosses games (the party apps). The
//! numbers are not here: the cross-game table is the app's
//! (`chimera_sim::weapon::balance::Balance`, party_kit's
//! data/party_balance.toml: Halo's `outgoing` and `incoming`, a campaign
//! body's own incoming by its id, `halo_ce:elite`), with the host's switch
//! (`mp_balance`). Without the table (Halo's own app) nothing is scaled.
//!
//! - **Incoming** (another game's damage on a Halo body): it counts the
//!   table's incoming k (`incoming_of`, by the body's id, `body_id`)
//!   against the shield and the body, the shield still first. The engine's
//!   armour step stays as it is for every game (`weapon::hits::absorb`):
//!   the shield's `weapon::Armor` holds the shield fraction divided by k
//!   (`shield_unit`: a full shield at k = 1.5 is 1 / 1.5 of another game's
//!   armour, so it breaks after 66.7 of its damage, which is 100 counted),
//!   and the body's `core::Health` takes another game's damage times k
//!   (`core::DamageScale` on the body, applied by the engine's
//!   `apply_damage`; Halo's own hits are written divided by it, and count
//!   the table's scale themselves, `own_scale`: Halo's outgoing times the
//!   body's incoming, as any game's, with the switch on; the spec's
//!   numbers with it off). The body holds k so a hit that takes armour in
//!   a game's own step (Combat Arms') counts it too; the engine's armour
//!   step (`weapon::HitArmor`) applies the rest of the pair's scale, so a
//!   hit is scaled once. `keep_incoming` keeps k the table's for the body
//!   (a campaign body forced on, the switch put in force at a match's
//!   start), the shield's value kept exactly across a change.
//! - **Outgoing** (a Halo source on another game's character): Halo's
//!   conversion (a Halo point 1/75 of that character's health,
//!   `super::damage`), then the table's outgoing of Halo, in the engine's
//!   armour step.
//! - **Raw** (the switch off): a body keeps k = 1 and Halo's guns deal
//!   by Halo's conversion. Where no game's own answer exists, the
//!   mechanism is the answer: another game's hit on a Halo shield (a CS:S
//!   bullet) is that game's damage as it deals it, taken by the shield as
//!   Halo's armour (every part, the whole hit until the shield is empty,
//!   the overflow to the body; a full shield is 100 of its points, the
//!   Spartan's 75 shield points), with none of Halo's material
//!   multipliers; a Halo gun on another game's body counts that body as an
//!   unshielded Spartan's (material type 21).
//! - **The armour brought in** (`Brought`): a character becoming Halo's
//!   keeps today's rule (what armour it brings is the shield's start,
//!   docs/DESIGN.md "Armour when the identity switches"); one that stops
//!   being Halo's gets back what it brought (its value and kind, or none),
//!   never the shield's value (`super::vitality`). A behaviour, not a
//!   number: the same with the switch on and off.
//! - **The power reader's hand** (`PowerHand`, data/power_hand.toml): the
//!   taps a second a trigger with no rate limit fires at, and the bursts
//!   an automatic gun with growing error is fired in.

use bevy::prelude::*;
use chimera_sim::weapon::{Armor, balance::Balance};
use serde::Deserialize;

use super::{campaign::BodyKind, vitality::Mirrored, vitality::Vitality};
use crate::core::DamageScale;

/// The power reader's hand, as shipped.
pub const DATA: chimera_core::game_data::GameData = chimera_core::game_data!("data/power_hand.toml");

/// What the power reader assumes of a hand (data/power_hand.toml).
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PowerHand {
    pub hand_taps_per_second: f32,
    pub burst_seconds: f32,
}

impl PowerHand {
    /// The shipped data. Bad built-in data panics.
    pub fn load() -> Self {
        toml::from_str(&DATA).unwrap_or_else(|e| panic!("halo_ce data/power_hand.toml: {e}"))
    }
}

/// The body id of a Halo body (the sandbox's: `halo_ce:spartan`, a
/// campaign kind's `halo_ce:elite`).
pub fn body_id(kind: Option<&BodyKind>) -> &'static str {
    kind.map_or("halo_ce:spartan", |k| super::campaign::targets::target_id(*k).0)
}

/// The incoming another game's damage counts on Halo body `body` (its
/// id) under the table `balance` (1: none, or the switch off).
pub fn incoming_of(balance: Option<&Balance>, body: &str) -> f32 {
    balance.map_or(1.0, |b| b.incoming(Some(super::GAME), Some(body)))
}

/// The scale of a Halo hit on a Halo body of `kind` (None: the Spartan)
/// under the table `balance`: Halo's outgoing times that body's incoming
/// (`Balance::pair`, the same one every game's hit gets; 1: no table, or
/// the switch off).
pub fn own_scale(balance: Option<&Balance>, kind: Option<&BodyKind>) -> f32 {
    balance.map_or(1.0, |b| b.pair(super::GAME, Some(super::GAME), Some(body_id(kind))))
}

/// `incoming_of` with the world's table.
pub fn incoming_in(world: &World, body: &str) -> f32 {
    incoming_of(world.get_resource::<Balance>(), body)
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
/// it gets back when it stops being Halo's. Put on and taken off with the
/// rest of Halo's parts, in the same move.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
#[component(storage = "SparseSet")]
pub struct Brought(pub Option<Armor>);

/// Each Halo body's held incoming is the table's for it (module doc): a
/// change converts the shield's `Armor` (and what the vitality last wrote
/// of it) to the new unit, so the shield's value stays.
#[allow(clippy::type_complexity)]
pub fn keep_incoming(
    mut q: Query<(Entity, Option<&BodyKind>, Option<&DamageScale>, Option<&mut Armor>, Option<&mut Mirrored>), With<Vitality>>,
    balance: Option<Res<Balance>>,
    mut commands: Commands,
) {
    for (e, kind, held, armor, mirrored) in &mut q {
        let want = incoming_of(balance.as_deref(), body_id(kind));
        let have = super::vitality::scale_of(held);
        if want == have {
            continue;
        }
        // Unprimed, the shield's value is not yet in a unit (the first
        // update takes `Armor` as it is).
        if let Some(mut m) = mirrored.filter(|m| m.primed) {
            m.shield = shield_unit(shield_of(m.shield, have), want);
            if let Some(mut a) = armor {
                a.amount = shield_unit(shield_of(a.amount, have), want);
            }
        }
        // Set in place (on every character from its spawn; 1: none).
        commands.entity(e).insert(DamageScale(want));
    }
}
