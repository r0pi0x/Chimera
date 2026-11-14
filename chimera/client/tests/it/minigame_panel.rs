//! The minigame panel (`client::minigame_panel`) names a rung's gun by its
//! full name; a status line that would run wider than the panel's line
//! (`LINE_MAX`) names it by its short name (`GunNames`), fitted.

use bevy::prelude::*;
use chimera_client::client::{
    minigame_panel::{MinigamePanelPlugin, PanelLine},
    minigame_scoreboard::GunNames,
};
use chimera_sim::{
    SimBasePlugins,
    minigame::hud::{HudLine, MinigameHud},
};
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim};

fn status(sim: &mut Sim) -> Vec<String> {
    let w = sim.app.world_mut();
    w.query::<(&PanelLine, &Text)>().iter(w).filter(|(k, _)| **k == PanelLine::Status).map(|(_, t)| t.0.clone()).collect()
}

#[test]
fn a_line_too_wide_for_the_gun_names_it_short() {
    let mut sim = Sim::new(SimBasePlugins, (GreyboxMapPlugin, MinigamePanelPlugin));
    let mut names = GunNames::from_full([
        ("kit:rifle".to_string(), "Field Rifle".to_string()),
        (
            "kit:long".to_string(),
            "Sun God Ra Engraved Commemorative Double Barreled Rifle (Mythic Anniversary Edition)".to_string(),
        ),
    ]);
    names.short.insert("kit:rifle".into(), "Rifle".into());
    names.short.insert("kit:long".into(), "Ra Rifle DB".into());
    sim.app.insert_resource(names);
    let line = |w: &'static str| HudLine { text: "Gun: {weapon}".into(), weapon: Some(w), ..default() };
    sim.app.insert_resource(MinigameHud { title: "Kit".into(), lines: vec![line("kit:rifle"), line("kit:long")], ..default() });
    sim.ticks(2);
    let mut got = status(&mut sim);
    got.sort();
    assert_eq!(got, ["Gun: Field Rifle", "Gun: Ra Rifle DB"], "the full name where it fits, the short one where not");
}
