//! TF2's rung passives (docs/plans/active/rung-passives.md, P1; Chimera's
//! `minigame::passives`): the worn items a TF2 rung may carry beside its
//! gun when the host has passives on (off by default), from
//! data/rung_passives.toml (`TABLE`): the Soldier's Mantreads, the
//! Demoman's boots, the Sniper's Darwin's Danger Shield, Cozy Camper and
//! Razorback. Drawn with the rung's class (`classes::HOOK`'s draw: the
//! drawn class, else the gun's own) under Chimera's conflict rule (the
//! item's loadout slot never the gun's for that class: a Soldier on the
//! Shotgun gets no boots); given by the hook's `apply` (`apply`:
//! `passives::wear`), worn through `tf2:worn` (`Unselectable`), taken when
//! the player leaves the rung. An item the item pack lacks is never drawn.
//!
//! Known shortcuts: no keyed items yet (shields, watches, the B.A.S.E.
//! Jumper, the Thermal Thruster: P3); the Gunboats aren't an item of
//! ours yet (their own-blast reduction while worn isn't built); worn
//! items aren't drawn on the body (P4).

use bevy::prelude::*;
use chimera_core::game_data::GameData;
use chimera_sim::minigame::{
    auto::Rng,
    ladder::{RungDraw, RungExtras},
    passives::{self, PassiveTable, RungGun},
};
use chimera_sim::weapon::data::WeaponCatalog;

/// The table, as written.
pub const DATA: GameData = chimera_core::game_data!("data/rung_passives.toml");

/// TF2's passives table (`DATA`, parsed).
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct Tf2Passives(pub PassiveTable);

impl Default for Tf2Passives {
    fn default() -> Self {
        Tf2Passives(PassiveTable::parse(&DATA).unwrap_or_else(|e| panic!("data/rung_passives.toml: {e}")))
    }
}

/// Rung `d`'s passives for class `class` (none while off), drawn on `rng`.
pub fn draw(world: &World, d: &RungDraw, class: &str, rng: &mut Rng) -> Vec<String> {
    let table = world.get_resource::<Tf2Passives>().cloned().unwrap_or_default().0;
    let data_slot = world.get_resource::<WeaponCatalog>().and_then(|c| c.get(d.weapon).map(|w| w.slot)).unwrap_or(0);
    let gun = RungGun {
        class,
        slot: table.gun_slot(d.weapon, class, data_slot),
        keys: table.gun_keys(d.weapon),
    };
    passives::draw(&table, d, &gun, rng, &|id| passives::registered(world, id))
}

/// Player `e` armed for a rung (`classes::HOOK`'s `apply`): its TF2
/// passives worn, the others taken.
pub fn apply(world: &mut World, e: Entity, extras: Option<&RungExtras>) {
    let ids = extras.map(|x| x.passives.clone()).unwrap_or_default();
    passives::wear(world, e, super::GAME, &ids);
}
