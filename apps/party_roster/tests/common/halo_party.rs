//! The party playing a ladder with Halo's guns, a body of each game.

#![allow(dead_code, unused_imports)]

use crate::play::*;
use bevy::prelude::*;
use chimera_client::client::{kill_feed::KillFeedStatePlugin, minigame_scoreboard::gun_name};
use chimera_core::{
    core::{Died, Health, Team},
    slots::{GameIdentity, MovementSlot},
};
use chimera_sim::{
    minigame::{self, MinigameAppExt, ladder::Ladder},
    mode::state::{Match, Phase},
    weapon::{Armor, ArmorPieces, ArmorRatio, PiecesOf, data::WeaponCatalog},
};
use chimera_world::{
    greybox::GreyboxMapPlugin,
    harness::Sim,
    map::{BodyModel, CharacterModels, Held, MapCharacterModel, MapSimPlugin},
};
use combat_arms::harness::CaSim;
use halo_ce::game::{
    movement::{self as halo_movement, HaloMovement},
    vitality::{Stun, Vitality, shield_armour},
    weapons,
};
use party_roster::PartyPartsPlugin;
use crate::play::{alive, held, line_up, no_finale, phase, plain_rungs, rung, shoot_until_dead, soldier};

/// The stand-in bodies, by game.
pub(crate) const BODIES: [&str; 3] = ["combat_arms", "cs_source", "halo_ce"];

/// The party playing the ladder minigame `text` (`test:gun_game_hl2dm`)
/// with a body of each game.
pub(crate) fn party_with(text: &str) -> Sim {
    let mut sim = Sim::with(CaSim, |app| {
        app.add_plugins((GreyboxMapPlugin, MapSimPlugin::empty(), PartyPartsPlugin, KillFeedStatePlugin));
    });
    let body = |game| MapCharacterModel {
        game: Some(game),
        team: None,
        name: None,
        class: None,
        model: default(),
        hitboxes: Vec::new(),
        bones: Vec::new(),
        root: Transform::IDENTITY,
        animations: None,
        boxes: Vec::new(),
        ragdoll: None,
    };
    sim.app.insert_resource(CharacterModels(std::sync::Arc::new(BODIES.iter().map(|g| body(*g)).collect())));
    plain_rungs(sim.app.world_mut());
    no_finale(sim.app.world_mut());
    sim.app.add_minigames(&[("halo_ladder.toml", text)]).unwrap();
    minigame::start(sim.app.world_mut(), "test:gun_game_hl2dm").expect("the gun game starts");
    sim
}
