//! TF2's class bodies in a match's asset offer (`bodies::asset_names`,
//! an `AssetHooks` entry; `chimera_net::net::assets::match_sets`, which
//! the server's offer and a server reading TF2 from the store both use):
//! a gun game whose ladder holds only the Heavy's and the Scout's guns
//! offers `players/heavy` and `players/scout` beside TF2's `base`,
//! `players` and `common`, and no other class's set; a class whose data
//! names another in `needs` (as the Spy will every class, once the
//! disguise exists) pulls that class's set in too. On TF2's own headless
//! sim (`Tf2Sim`) with Chimera's minigames and network on top.

use bevy::prelude::*;
use chimera_core::{game_registry::Games, slots::Loadout};
use chimera_net::{
    harness::NetSim,
    minigame_net::MinigameNetPlugin,
    net::{
        assets::AssetHost,
        manifest::{self, MatchManifest},
        memory::LinkConditions,
    },
};
use chimera_sim::{
    minigame::{self, MinigameAppExt, MinigamePlugin},
    movement::placeholder,
};
use chimera_world::greybox::GreyboxMapPlugin;
use tf2::{
    game::{
        INFO,
        classes::{self, Classes},
    },
    harness::Tf2Sim,
};

const DEF: &str = r#"
id = "kit:heavy_scout"
name = "Heavy and Scout"
players = { min = 2, max = 4 }
maps = ["chimera:greybox"]
teams = "free_for_all"
time_limit = "60s"
rotate = false

[spawn]
respawn = { delay = "1s", protection = "0.5s" }
points = "map"

[rules]
module = "chimera:ladder"
ladder = ["tf2:tf_weapon_minigun", "tf2:tf_weapon_scattergun", "tf2:tf_weapon_pistol_scout"]
"#;

/// A server of the gun game with TF2's classes `table`, its offer now.
fn offer_with(table: Option<Classes>) -> MatchManifest {
    let mut sim = NetSim::new(Tf2Sim, LinkConditions::default(), 3, 0, move |app| {
        app.add_plugins((GreyboxMapPlugin, MinigamePlugin, MinigameNetPlugin))
            .insert_resource(Loadout { movement: placeholder::ID });
        app.world_mut().get_resource_or_init::<Games>().register(&INFO);
        app.add_minigames(&[("kit_heavy_scout.toml", DEF)]).expect("the definition reads");
        app.insert_resource(AssetHost::new("https://assets.invalid:1", "ab".repeat(32)));
        if let Some(t) = table.clone() {
            app.insert_resource(t);
        }
    });
    minigame::start(sim.server.app.world_mut(), "kit:heavy_scout").expect("it starts");
    sim.ticks(10);
    manifest::current(sim.server.app.world_mut())
}

/// The offer's sets of TF2.
fn tf2_sets(m: &MatchManifest) -> Vec<String> {
    let offer = m.assets.as_ref().expect("an offer");
    offer.now.iter().filter(|g| g.game == "tf2").flat_map(|g| g.sets.clone()).collect()
}

fn class_sets(sets: &[String]) -> Vec<&str> {
    sets.iter().filter(|s| s.starts_with("players/")).map(String::as_str).collect()
}

#[test]
fn a_heavy_and_scout_ladder_offers_only_their_class_sets() {
    let sets = tf2_sets(&offer_with(None));
    for s in ["base", "players", "common", "weapon/tf_weapon_minigun", "weapon/tf_weapon_scattergun"] {
        assert!(sets.iter().any(|x| x == s), "{s} in {sets:?}");
    }
    // The Scout's two guns name his set once; nobody else's.
    assert_eq!(class_sets(&sets), ["players/scout", "players/heavy"], "{sets:?}");
}

#[test]
fn a_class_that_needs_another_pulls_its_set_in() {
    // The Heavy shows the Medic's body too (a stand-in for the Spy's
    // disguise, which will need every class): data only.
    let text = classes::DATA.replace("arms = \"heavy\"", "arms = \"heavy\"\nneeds = [\"medic\"]");
    let table = Classes::parse(&text).expect("the table reads");
    let sets = tf2_sets(&offer_with(Some(table)));
    assert_eq!(class_sets(&sets), ["players/scout", "players/medic", "players/heavy"], "{sets:?}");
}
