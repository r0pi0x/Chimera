//! What a match's asset offer names (`net::assets::offer`, its weapons
//! from `minigame_net::match_weapons`): each weapon's set under the name
//! `chimera_pack sets` publishes (`weapon/<id>`, without the `weapon_`
//! Source ids start with), and, besides the definition's ladder and kit,
//! what the host drew for the playing ladder (its rungs' extras: a melee
//! weapon, a character's weapon), so a client fetches those with the
//! match instead of one by one while it plays.

use bevy::prelude::*;
use chimera_core::slots::Loadout;
use chimera_net::{
    harness::NetSim,
    minigame_net::{MinigameNetPlugin, match_weapons},
    net::{
        assets::{self, AssetHooks, AssetHost},
        manifest::MatchManifest,
        memory::LinkConditions,
    },
};
use chimera_sim::{
    SimBasePlugins,
    minigame::{
        self, MinigameAppExt, MinigamePlugin,
        ladder::{Ladder, RungCharacter, RungExtras},
    },
    movement::placeholder,
    testkit,
};
use chimera_world::greybox::GreyboxMapPlugin;

const DEF: &str = r#"
id = "kit:offer_ladder"
name = "Kit gun game"
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
ladder = ["kit:pistol", "kit:rifle", "kit:shotgun"]
"#;

fn setup(app: &mut App) {
    app.add_plugins((GreyboxMapPlugin, MinigamePlugin, MinigameNetPlugin)).insert_resource(Loadout {
        movement: placeholder::ID,
    });
    testkit::add_weapons(app);
    app.add_minigames(&[("kit_offer_ladder.toml", DEF)]).expect("the definition reads");
}

/// The offer's sets of `game` now.
fn sets_of(m: &MatchManifest, game: &str) -> Vec<String> {
    let offer = m.assets.as_ref().expect("an offer");
    offer.now.iter().filter(|g| g.game == game).flat_map(|g| g.sets.clone()).collect()
}

/// A Source weapon's set is offered once, as published (`weapon/ak47`),
/// not also as `weapon/weapon_ak47`, which no service has; other ids keep
/// their name.
#[test]
fn the_offer_names_weapon_sets_as_published() {
    let mut world = World::new();
    world.insert_resource(AssetHost::new("https://assets.invalid:1", "ab".repeat(32)));
    world.init_resource::<AssetHooks>();
    world
        .resource_mut::<AssetHooks>()
        .0
        .push(|_| (vec!["src:weapon_ak47".into(), "lt:m9_knife".into()], Vec::new()));
    let mut m = MatchManifest { map: "src:de_test".into(), ..default() };
    assets::offer(&mut world, &mut m);
    assert_eq!(sets_of(&m, "src"), ["base", "players", "map/de_test", "weapon/ak47"]);
    assert_eq!(sets_of(&m, "lt"), ["base", "players", "weapon/m9_knife"]);
}

/// The weapons the playing ladder's extras hand out (a rung's melee
/// weapon, a rung character's weapon: the host's draw, not in the
/// definition) are the match's weapons too, once each.
#[test]
fn the_ladders_drawn_weapons_are_the_matchs() {
    let mut sim = NetSim::new(SimBasePlugins, LinkConditions::default(), 3, 0, setup);
    minigame::start(sim.server.app.world_mut(), "kit:offer_ladder").expect("it starts");
    sim.ticks(10);
    let world = sim.server.app.world_mut();
    let before = match_weapons(world);
    assert!(before.contains(&"kit:rifle".to_string()), "{before:?}");
    let mut ladder = world.resource_mut::<Ladder>();
    let rungs = ladder.rungs.len();
    ladder.extras = vec![None; rungs];
    ladder.extras[0] = Some(RungExtras { game: "lt".into(), melee: Some("lt:baton".into()), ..default() });
    ladder.extras[1] = Some(RungExtras {
        game: "lt".into(),
        melee: Some("lt:baton".into()),
        character: Some(RungCharacter { id: "lt:infected".into(), weapon: "lt:claws".into() }),
        ..default()
    });
    let after = match_weapons(sim.server.app.world());
    for w in ["kit:pistol", "kit:rifle", "kit:shotgun", "lt:baton", "lt:claws"] {
        assert_eq!(after.iter().filter(|x| *x == w).count(), 1, "{w} once in {after:?}");
    }
}
