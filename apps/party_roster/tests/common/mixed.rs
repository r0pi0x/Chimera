//! The mixed ladder: HL2:DM's guns between CS:S's and Combat Arms'.

#![allow(dead_code, unused_imports)]

use crate::play::*;
use bevy::prelude::*;
use chimera_core::{
    core::Velocity,
    slots::{GameIdentity, MovementSlot},
};
use chimera_sim::{
    minigame::{self, MinigameAppExt, ladder::Ladder},
    testkit,
    weapon::{Inventory, Weapon},
};
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim, map::{GameViewModelFovs, ViewModelSettings, view_model::view_model_fov}};
use combat_arms::harness::CaSim;
use hl2dm::game::{
    movement as hl2_movement,
    rungs::RungModel,
    weapons::{self as hl2, inventory::Hl2Model},
};
use party_roster::PartyPartsPlugin;
use crate::play::{held, line_up, no_finale, shoot_until_dead, soldier};

/// The ladder of `MIXED`: HL2:DM's pistol first, then CS:S's, Combat
/// Arms' and HL2:DM's again.
pub(crate) const LADDER: [&str; 5] =
    [hl2::PISTOL, "cs_source:weapon_glock", hl2::SMG1, "combat_arms:beretta_m92fs", hl2::SHOTGUN];

/// The party's gun game with `LADDER` and every game's knife.
pub(crate) fn mixed_text(rung_extras: bool) -> String {
    ladder_text(&LADDER, rung_extras)
}
