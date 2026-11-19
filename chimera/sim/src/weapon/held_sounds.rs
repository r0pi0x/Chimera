//! Sounds a weapon makes only while it is in its holder's hands (any
//! game): a gun's loops (TF2's Minigun spinning and firing, its Flame
//! Thrower's flame) name their entries in `HeldSounds` on the weapon, and
//! when the weapon stops being the living holder's drawn one, for any
//! reason (switched away, taken by a gun game's rung swap, stripped,
//! dropped, despawned, the holder dead), `stop_out_of_hand` sends a
//! `map::StopSound` for each, so none plays on under the holder.
//!
//! Each machine stops what it can see: the server (and a listen host) for
//! every holder, a network server's `StopSound` going on to every client
//! but the holder's; the holder's client for its own player, from its own
//! inventory (its prediction). Only those entries stop: a wind-down the
//! game plays on the same channel as the switch happens is left alone.
//! Run in `Update`, after the frame's ticks.

use bevy::prelude::*;

use super::Inventory;
use crate::{core::Health, map::StopSound};

/// The sound entries this weapon plays only while drawn (its loops).
#[derive(Component, Clone, Debug, Default, PartialEq, Eq)]
pub struct HeldSounds(pub Vec<String>);

/// On a holder: the drawn weapon with `HeldSounds` and its entries, as
/// last seen.
#[derive(Component, Clone, Debug, PartialEq, Eq)]
#[component(storage = "SparseSet")]
pub struct SoundsInHand {
    pub weapon: Entity,
    pub entries: Vec<String>,
}

/// Stop the held sounds of each weapon that left its holder's hands
/// (module doc).
#[allow(clippy::type_complexity)]
pub fn stop_out_of_hand(
    holders: Query<(Entity, Option<&Inventory>, Option<&Health>, Option<&SoundsInHand>), Or<(With<Inventory>, With<SoundsInHand>)>>,
    weapons: Query<&HeldSounds>,
    mut stops: MessageWriter<StopSound>,
    mut commands: Commands,
) {
    for (holder, inv, health, before) in &holders {
        let alive = health.is_none_or(|h| h.current > 0.0);
        let now = inv
            .and_then(|i| i.active)
            .filter(|w| alive && inv.is_some_and(|i| i.weapons.contains(w)))
            .and_then(|w| weapons.get(w).ok().map(|s| (w, s)));
        match (before, now) {
            (Some(b), Some((w, _))) if b.weapon == w => {}
            (before, now) => {
                if let Some(b) = before {
                    for entry in &b.entries {
                        stops.write(StopSound { by: Some(holder), source: holder, entry: entry.clone() });
                    }
                }
                match now {
                    Some((w, s)) => {
                        commands.entity(holder).insert(SoundsInHand { weapon: w, entries: s.0.clone() });
                    }
                    None => {
                        commands.entity(holder).remove::<SoundsInHand>();
                    }
                }
            }
        }
    }
}
