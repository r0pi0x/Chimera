//! What a match's asset offer names (`net::assets::offer`, its weapons
//! from `minigame_net::match_weapons`): each weapon's set under the name
//! `chimera_pack sets` publishes (`weapon/<id>`, without the `weapon_`
//! Source ids start with), and, besides the definition's ladder and kit,
//! what the host drew for the playing ladder (its rungs' extras: a melee
//! weapon, a character's weapon), so a client fetches those with the
//! match instead of one by one while it plays; and the sets a game's own
//! `AssetHooks` entry names for its rung extras (Combat Arms' drawn
//! bodies, `character/<record>`, `combat_arms::game::rungs`). While a
//! rotation changes map (`MapChange`), the offer names the coming
//! ladder's draw (`minigame_net::match_ladder`, `ladder::draw`), the one
//! its match then deals.

use bevy::prelude::*;
use chimera_core::{
    game_registry::{GameInfo, Games},
    slots::Loadout,
};
use chimera_net::{
    harness::NetSim,
    minigame_net::{MapChange, MinigameNetPlugin, change_match, match_ladder, match_weapons},
    net::{
        assets::{self, AssetHooks, AssetHost, AssetNames},
        manifest::{self, MatchManifest},
        memory::LinkConditions,
    },
};
use chimera_sim::{
    SimBasePlugins,
    minigame::{
        self, MinigameAppExt, MinigamePlugin,
        auto::Rng,
        ladder::{self, Ladder, RungCharacter, RungDraw, RungExtras, RungExtrasAppExt, RungExtrasHook, RungSeed},
    },
    movement::placeholder,
    testkit,
    weapon::data::WeaponDataAppExt,
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
        .push(|_| AssetNames {
            weapons: vec!["src:weapon_ak47".into(), "lt:m9_knife".into()],
            ..default()
        });
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

/// A stand-in for a game with rung bodies (`lt`): known to the server.
static LT: GameInfo = GameInfo {
    id: "lt",
    short_name: "LT",
    content_dir: "lt",
    tick_interval: 0.02,
    open: |_| Err(std::io::ErrorKind::NotFound.into()),
    map_cycle: |_| Vec::new(),
    map_names: |_| Vec::new(),
    map_file: |_| None,
    steam: None,
    import_map: |_| Err("no".into()),
    default_binds_cfg: "cfg/default.cfg",
    default_binds: &[],
    old_default_binds: &[],
    movement_cvars: &[],
    meters_per_unit: 0.018,
    console_namespaces: &[],
};

/// The stand-in's `AssetHooks` entry, as Combat Arms' (`rungs::
/// asset_names`): the body each of the match's ladder's rungs' extras
/// drew (`model`), as that body's `character/<record>` set.
fn rung_bodies(world: &World) -> AssetNames {
    let sets = match_ladder(world)
        .iter()
        .flat_map(|l| l.extras.iter().flatten())
        .filter_map(|x| x.values.iter().find(|(k, _)| k == "model"))
        .map(|(_, body)| format!("lt:character/{body}"))
        .collect();
    AssetNames { sets, ..default() }
}

/// The sets a game names for what its rung extras drew (a rung's body:
/// the host's draw, not in the definition) are in the match's offer now,
/// once each, beside the game's `base` and `players`: a client fetches
/// them with the match, not file by file when a player reaches the rung.
#[test]
fn the_offer_names_the_ladders_drawn_bodies() {
    let mut sim = NetSim::new(SimBasePlugins, LinkConditions::default(), 3, 0, |app| {
        setup(app);
        app.world_mut().get_resource_or_init::<Games>().register(&LT);
        app.world_mut().resource_mut::<AssetHooks>().0.push(rung_bodies);
        app.insert_resource(AssetHost::new("https://assets.invalid:1", "ab".repeat(32)));
    });
    minigame::start(sim.server.app.world_mut(), "kit:offer_ladder").expect("it starts");
    sim.ticks(10);
    let world = sim.server.app.world_mut();
    let none = manifest::current(world);
    assert!(none.assets.as_ref().is_none_or(|o| o.now.iter().all(|g| g.game != "lt")), "no draw, no lt: {none:?}");
    let mut ladder = world.resource_mut::<Ladder>();
    let rungs = ladder.rungs.len();
    ladder.extras = vec![None; rungs];
    let body = |b: &str| Some(RungExtras { game: "lt".into(), values: vec![("model".into(), b.into())], ..default() });
    ladder.extras[0] = body("17");
    ladder.extras[1] = body("42");
    ladder.extras[2] = body("17");
    let m = manifest::current(world);
    assert_eq!(sets_of(&m, "lt"), ["base", "players", "character/17", "character/42"]);
}

/// A ladder with a knife finale and rung extras: the host draws its melee
/// rungs' knives and, per gun rung, a melee weapon and a body.
const ROTATION_DEF: &str = r#"
id = "kit:offer_rotation"
name = "Kit gun game"
players = { min = 2, max = 4 }
maps = ["chimera:greybox", "lt:dm_next"]
teams = "free_for_all"
time_limit = "60s"

[spawn]
respawn = { delay = "1s", protection = "0.5s" }
points = "map"

[rules]
module = "chimera:ladder"
ladder = ["kit:pistol", "kit:rifle", "kit:shotgun"]
knife_finale = true
melee_rungs = 2
"#;

/// The test kit as a game the server knows (its weapons' sets offered).
static KIT: GameInfo = GameInfo {
    id: "kit",
    short_name: "Kit",
    content_dir: "kit",
    ..LT
};

const KNIVES: [&str; 6] = ["kit:knife_a", "kit:knife_b", "kit:knife_c", "kit:knife_d", "kit:knife_e", "kit:knife_f"];

fn knife(id: &str) -> String {
    format!(
        "id = \"{id}\"\nbase = \"kit:base\"\nslot = 2\ndraw = 0.5\n\n[melee.primary]\nrange = 2.0\nhull = 0.5\n\
         facing_cos = 0.7\ndamage = 50.0\nhit_refire = 0.5\nmiss_refire = 0.4\nmiss_refire_other = 0.5\nforce = 0.0\n"
    )
}

/// The kit's stand-in draw: each gun rung a melee weapon and a body.
fn draw(_: &World, d: &RungDraw, rng: &mut Rng) -> Option<RungExtras> {
    (!d.melee).then(|| RungExtras {
        melee: Some(KNIVES[rng.below(KNIVES.len())].to_string()),
        values: vec![("model".into(), (rng.next() % 100_000).to_string())],
        ..default()
    })
}

const HOOK: RungExtrasHook = RungExtrasHook {
    game: "kit",
    options: &[],
    melee: |_| Vec::new(),
    characters: |_| Vec::new(),
    draw,
    apply: |_, _, _| {},
};

/// What a ladder's draw puts in the offer: its melee rungs' and extras'
/// melee weapons as `kit` weapon sets, its bodies as `lt` character sets.
fn drawn_sets(l: &Ladder) -> (Vec<String>, Vec<String>) {
    let mut weapons: Vec<String> = l.rungs[l.rungs.len() - l.melee_rungs..].iter().map(|w| w.to_string()).collect();
    let mut bodies = Vec::new();
    for x in l.extras.iter().flatten() {
        weapons.extend(x.melee.clone());
        bodies.extend(x.values.iter().filter(|(k, _)| k == "model").map(|(_, b)| format!("character/{b}")));
    }
    let set = |w: &String| format!("weapon/{}", w.strip_prefix("kit:").unwrap());
    (weapons.iter().map(set).collect(), bodies)
}

/// A rotation to another map (`change_match`, as `go_to` does) deals a
/// new ladder only once the map is in, but the offer of the level change
/// (built while the map changes, `MapChange`) already names that
/// ladder's drawn melee weapons and bodies, not the old ladder's: a
/// client fetches them with the level change, not one by one in the
/// match. The match then deals exactly that draw.
#[test]
fn a_rotations_offer_names_the_coming_ladders_draw() {
    let mut sim = NetSim::new(SimBasePlugins, LinkConditions::default(), 3, 0, |app| {
        app.add_plugins((GreyboxMapPlugin, MinigamePlugin, MinigameNetPlugin)).insert_resource(Loadout {
            movement: placeholder::ID,
        });
        testkit::add_weapons(app);
        let docs: Vec<(String, String)> = KNIVES.iter().map(|id| (format!("{id}.toml"), knife(id))).collect();
        let docs: Vec<(&str, &str)> = docs.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
        app.add_weapon_data(&docs).expect("the knives load");
        app.add_rung_extras(HOOK).insert_resource(RungSeed(217_377));
        app.add_minigames(&[("kit_offer_rotation.toml", ROTATION_DEF)]).expect("the definition reads");
        let mut games = app.world_mut().get_resource_or_init::<Games>();
        games.register(&LT);
        games.register(&KIT);
        app.world_mut().resource_mut::<AssetHooks>().0.push(rung_bodies);
        app.insert_resource(AssetHost::new("https://assets.invalid:1", "ab".repeat(32)));
    });
    minigame::start(sim.server.app.world_mut(), "kit:offer_rotation").expect("it starts");
    sim.ticks(10);
    let world = sim.server.app.world_mut();
    let old = world.resource::<Ladder>().clone();
    let (old_weapons, old_bodies) = drawn_sets(&old);
    assert!(!old_bodies.is_empty() && old_weapons.len() > 2, "the stand-in draws: {old:?}");

    change_match(world, "kit:offer_rotation", Some("lt:dm_next")).expect("the rotation starts");
    assert!(world.contains_resource::<MapChange>(), "a map change is under way");
    let def = world.resource::<MapChange>().def;
    let coming = ladder::draw(world, def).expect("a ladder to come");
    assert_ne!(coming, old, "a new ladder's draw");
    let (weapons, bodies) = drawn_sets(&coming);
    let m = manifest::current(world);
    let kit = sets_of(&m, "kit");
    let lt = sets_of(&m, "lt");
    for w in &weapons {
        assert!(kit.contains(w), "the coming ladder's {w} is offered: {kit:?}");
    }
    for b in &bodies {
        assert!(lt.contains(b), "the coming ladder's {b} is offered: {lt:?}");
    }
    for b in old_bodies.iter().filter(|b| !bodies.contains(b)) {
        assert!(!lt.contains(b), "the old ladder's {b} isn't: {lt:?}");
    }

    // The match starts there once the map is in (`follow_map_change`:
    // `minigame::start`) and deals the offered draw.
    world.remove_resource::<MapChange>();
    minigame::start(world, "kit:offer_rotation").expect("it starts");
    assert_eq!(*world.resource::<Ladder>(), coming, "the match deals the ladder its offer named");
}
