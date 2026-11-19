//! Sounds a weapon makes only while it is in its holder's hands (any
//! game): a gun's loops (TF2's Minigun spinning and firing, its Flame
//! Thrower's flame) name their entries in `HeldSounds` on the weapon, and
//! when the weapon stops being its living holder's drawn one, for any
//! reason (switched away, dropped, the holder dead: `stop_out_of_hand`;
//! taken by a gun game's rung swap, stripped, despawned: `stop_on_remove`),
//! a `map::StopSound` goes out for each, so none plays on under the
//! holder.
//!
//! Each machine stops what it can see: the server (and a listen host) for
//! every holder, a network server's `StopSound` going on to every client
//! but the holder's; the holder's client for its own player, from its own
//! inventory (its prediction). Only those entries stop: a wind-down the
//! game plays on the same channel as the switch happens is left alone.
//! Run in `Update`, after the frame's ticks, over the weapons that have
//! held sounds (few archetypes; the holder whose hands they were in is
//! kept on the weapon, so nothing comes and goes on characters).

use bevy::prelude::*;

use super::{Inventory, Weapon};
use crate::{core::Health, map::StopSound};

/// The sound entries this weapon plays only while drawn (its loops), and
/// whose hands it was last seen in.
#[derive(Component, Clone, Debug, Default, PartialEq, Eq)]
pub struct HeldSounds {
    pub entries: Vec<String>,
    pub in_hand: Option<Entity>,
}

impl HeldSounds {
    pub fn new(entries: Vec<String>) -> Self {
        Self { entries, in_hand: None }
    }

    fn stop(&self, holder: Entity, stops: &mut MessageWriter<StopSound>) {
        for entry in &self.entries {
            stops.write(StopSound { by: Some(holder), source: holder, entry: entry.clone() });
        }
    }
}

/// Stop the held sounds of each weapon that left its holder's hands while
/// it lives on (module doc).
pub fn stop_out_of_hand(
    mut weapons: Query<(Entity, &Weapon, &mut HeldSounds)>,
    holders: Query<(&Inventory, Option<&Health>)>,
    mut stops: MessageWriter<StopSound>,
) {
    for (w, weapon, mut held) in &mut weapons {
        let now = weapon.owner.filter(|o| {
            holders.get(*o).is_ok_and(|(inv, health)| {
                inv.active == Some(w) && inv.weapons.contains(&w) && health.is_none_or(|h| h.current > 0.0)
            })
        });
        if held.in_hand == now {
            continue;
        }
        if let Some(h) = held.in_hand {
            held.stop(h, &mut stops);
        }
        held.in_hand = now;
    }
}

/// A weapon in hand despawned (a rung's swap, a strip): its held sounds
/// stop.
pub fn stop_on_remove(remove: On<Remove<HeldSounds>>, weapons: Query<&HeldSounds>, mut stops: MessageWriter<StopSound>) {
    if let Ok(held) = weapons.get(remove.entity)
        && let Some(h) = held.in_hand
    {
        held.stop(h, &mut stops);
    }
}
