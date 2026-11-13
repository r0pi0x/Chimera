//! The shell's minimal HUD (`client::hud_minimal`) is up for a game that
//! lends no HUD (the test kit's stand-in, with the minimal HUD as the
//! fallback) with the held gun's ammo, and down while CS:S's or Combat
//! Arms' HUD is the gameplay's; the minigame's panel (the shell's overlay)
//! is up under it as under theirs.

use bevy::prelude::*;
use chimera_client::client::{
    hud::HudRoot,
    hud_minimal::{MinimalAmmo, MinimalHudPlugin},
    minigame_panel::{MinigamePanelPlugin, PanelLine},
};
use chimera_core::{
    core::LocalPlayer,
    game_parts::{MINIMAL_HUD, PartsRegistry},
    gameplay::Gameplay,
};
use chimera_sim::minigame::{self, ladder::Ladder};
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim, map::MapSimPlugin};
use combat_arms::{game::movement, harness::CaSim};
use party_kit::PartyPartsPlugin;
use ultimate_gun_game::{MINIGAME, UltimateGunGamePlugin};

struct StandIn;

impl Plugin for StandIn {
    fn build(&self, app: &mut App) {
        chimera_sim::testkit::add_stand_in_game(app);
    }
}

fn up(sim: &mut Sim) -> bool {
    let w = sim.app.world_mut();
    let v: Vec<Visibility> = w
        .query::<(&HudRoot, &Visibility)>()
        .iter(w)
        .filter(|(r, _)| r.0 == MINIMAL_HUD)
        .map(|(_, v)| *v)
        .collect();
    assert_eq!(v.len(), 1, "one root");
    v[0] != Visibility::Hidden
}

fn text<T: Component>(sim: &mut Sim) -> String {
    let w = sim.app.world_mut();
    w.query_filtered::<&Text, With<T>>().single(w).unwrap().0.clone()
}

#[test]
fn the_minimal_hud_shows_for_a_game_without_one_and_goes_down_for_the_others() {
    let mut sim = Sim::new(
        CaSim,
        (
            GreyboxMapPlugin,
            MapSimPlugin::empty(),
            PartyPartsPlugin,
            StandIn,
            UltimateGunGamePlugin { seed: 3, rungs: 64, ..Default::default() },
            MinimalHudPlugin,
            MinigamePanelPlugin,
        ),
    );
    sim.app.world_mut().resource_mut::<PartsRegistry>().fallback_hud = MINIMAL_HUD;
    minigame::start(sim.app.world_mut(), MINIGAME).expect("the automatic gun game starts");
    let a = sim.spawn_character(Vec3::new(0.0, 1.0, 0.0), movement::ID);
    sim.app.world_mut().entity_mut(a).insert(LocalPlayer);
    sim.seconds(1.5);
    let l = sim.app.world().resource::<Ladder>().clone();
    let rung_of = |game: &str| l.rungs.iter().position(|id| id.starts_with(&format!("{game}:"))).unwrap();
    for game in ["cs_source", "combat_arms"] {
        minigame::ladder::set_rung(sim.app.world_mut(), a, rung_of(game), &l);
        sim.seconds(1.2);
        assert_eq!(sim.app.world().resource::<Gameplay>().hud, game);
        assert!(!up(&mut sim), "{game}'s HUD is up, the minimal one down");
    }
    minigame::ladder::set_rung(sim.app.world_mut(), a, rung_of("kit"), &l);
    sim.seconds(1.2);
    assert_eq!(sim.app.world().resource::<Gameplay>().hud, MINIMAL_HUD);
    assert!(up(&mut sim));
    let ammo = text::<MinimalAmmo>(&mut sim);
    assert!(ammo.starts_with("kit:") && ammo.contains(" | "), "{ammo:?}");
    let w = sim.app.world_mut();
    let panel: Vec<(PanelLine, String)> = w.query::<(&PanelLine, &Text)>().iter(w).map(|(k, t)| (*k, t.0.clone())).collect();
    assert!(panel.contains(&(PanelLine::Title, "Ultimate Gun Game".into())) && panel.len() > 1, "{panel:?}");
}
