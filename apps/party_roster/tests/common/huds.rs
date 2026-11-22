//! The party with both HUDs (CS:S's and Combat Arms'), and what they show.

#![allow(dead_code, unused_imports)]

use crate::play::*;
use std::{collections::HashMap, sync::Arc};
use bevy::prelude::*;
use chimera_client::client::{
    callouts::{CalloutRoot, CalloutView, CalloutsPlugin},
    minigame_panel::MinigamePanelPlugin,
    fonts::UiFonts,
    hud::{GameDraws, HudDraws, HudLookOwner, HudRoot, HudSwitchPlugin},
};
use chimera_core::{core::LocalPlayer, gameplay::Gameplay};
use chimera_sim::minigame::{
    self, MinigameAppExt,
    ladder::{self, Ladder},
};
use chimera_world::{
    greybox::GreyboxMapPlugin,
    harness::Sim,
    map::hud::{ActiveHud, GameHud, HudCoord, HudPanel},
};
use combat_arms::{game::movement, harness::CaSim};
use combat_arms_ui::hud::CaHudStatePlugin;
use cs_source_ui::{
    game_hud::{GameHudPlugin, Part},
    minigame_hud::{MinigameHudPlugin, PanelLine},
};
use party_roster::{DEFAULT_MINIGAME, PartyPartsPlugin};
use crate::play::{held, line_up, shoot_until_dead};

/// A CS:S HUD look with health and armour panels, as a CS:S map's.
pub(crate) fn cs_look() -> ActiveHud {
    let mut hud = GameHud::default();
    for (name, x) in [("HudHealth", 16.0), ("HudArmor", 148.0)] {
        hud.panels.insert(
            name.into(),
            HudPanel {
                x: HudCoord::Start(x),
                y: HudCoord::End(42.0),
                wide: 102.0,
                tall: 36.0,
                background: Some([0, 0, 0, 96]),
                icon: Vec2::new(8.0, 2.0),
                digit: Vec2::new(50.0, 2.0),
                digit2: Vec2::ZERO,
                keys: HashMap::new(),
            },
        );
    }
    ActiveHud(Arc::new(hud), HashMap::new())
}

/// The party with both HUDs (Combat Arms' state and a stand-in root for
/// its drawn panels, CS:S's panels and the minigame's panel on it), a
/// local soldier and another, the gun game playing.
pub(crate) fn party_with_huds() -> (Sim, Entity, Entity) {
    party_with_huds_and(|_| {})
}

/// `party_with_huds` with `extra` plugins added too.
pub(crate) fn party_with_huds_and(extra: impl FnOnce(&mut App)) -> (Sim, Entity, Entity) {
    let mut sim = Sim::with(CaSim, |app| {
        extra(app);
        app.add_plugins((GreyboxMapPlugin, PartyPartsPlugin))
            .init_resource::<UiFonts>()
            .add_plugins((HudSwitchPlugin, CaHudStatePlugin, GameHudPlugin, MinigameHudPlugin, MinigamePanelPlugin))
            .insert_resource(HudLookOwner(cs_source::game::GAME))
            .insert_resource(cs_look());
        // Combat Arms' drawn HUD: a root and what it draws instead of the
        // shell's (`ui::hud::CaHudPlugin` spawns its roots so, with
        // `HudRoot`, and registers its `GameDraws`).
        app.world_mut().spawn((Node::default(), HudRoot(combat_arms::game::GAME)));
        app.insert_resource(HudDraws(vec![(combat_arms::game::GAME, combat_arms_ui::hud::DRAWS)]));
    });
    crate::play::plain_rungs(sim.app.world_mut());
    crate::play::no_finale(sim.app.world_mut());
    minigame::start(sim.app.world_mut(), DEFAULT_MINIGAME).expect("the gun game starts");
    let me = sim.spawn_character(Vec3::new(0.0, 1.0, 0.0), movement::ID);
    sim.app.world_mut().entity_mut(me).insert(LocalPlayer);
    let other = sim.spawn_character(Vec3::new(3.0, 1.0, 0.0), movement::ID);
    sim.seconds(1.5);
    (sim, me, other)
}

pub(crate) fn hud(sim: &Sim) -> &'static str {
    sim.app.world().resource::<Gameplay>().hud
}

/// Whether a root of `game`'s HUD shows.
pub(crate) fn shown(sim: &mut Sim, game: &str) -> bool {
    let w = sim.app.world_mut();
    let roots: Vec<Visibility> = w
        .query::<(&HudRoot, &Visibility)>()
        .iter(w)
        .filter(|(r, _)| r.0 == game)
        .map(|(_, v)| *v)
        .collect();
    assert!(!roots.is_empty(), "{game}'s HUD has roots");
    roots.iter().all(|v| *v != Visibility::Hidden)
}

/// The minigame panel's lines (the shell's overlay), as text.
pub(crate) fn cs_minigame_lines(sim: &mut Sim) -> Vec<(PanelLine, String)> {
    let w = sim.app.world_mut();
    w.query::<(&PanelLine, &Text)>().iter(w).map(|(k, t)| (*k, t.0.clone())).collect()
}

pub(crate) fn swap_to(sim: &mut Sim, me: Entity, rung: usize) {
    let w = sim.app.world_mut();
    let l = w.resource::<Ladder>().clone();
    ladder::set_rung(w, me, rung, &l);
    sim.seconds(0.5);
}
