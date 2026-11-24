//! A TF2 gun of several classes gets its class with the ladder's draw,
//! before the match (`game::classes::HOOK`, Chimera's rung extras): over
//! many seeds a Shotgun rung draws each of its four classes (Engineer,
//! Soldier, Pyro, Heavy) and a Minigun rung always the Heavy; the drawn
//! class is in the ladder's extras the server shares with its client, and
//! on both the rung's players (a client's own and a server-side one, as a
//! bot is) wear it: its body, top speed and max health; the match's asset
//! offer names only the drawn classes' body sets. On TF2's own headless
//! sim (`Tf2Sim`) with Chimera's minigames and network on top.

use std::collections::BTreeSet;

use bevy::prelude::*;
use chimera_core::{core::Health, game_registry::Games, slots::{GameIdentity, Loadout}};
use chimera_net::{
    harness::NetSim,
    minigame_net::MinigameNetPlugin,
    net::{
        assets::AssetHost,
        manifest::{self, MatchManifest},
        memory::LinkConditions,
    },
};
use chimera_sim::minigame::{
    self, MinigameAppExt, MinigamePlugin, Minigames,
    ladder::{self, Ladder, RungSeed},
};
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim, map::BodyName};
use source_engine::source_movement::MoveTraits;
use tf2::{
    game::{
        INFO,
        classes::{self, Classes, RungClass, Tf2Class},
        movement,
        weapons::{MINIGUN, SHOTGUN},
    },
    harness::Tf2Sim,
};

const DEF: &str = r#"
id = "kit:shotgun_heavy"
name = "Shotgun and Minigun"
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
ladder = ["tf2:tf_weapon_shotgun_primary", "tf2:tf_weapon_minigun"]
"#;

const ID: &str = "kit:shotgun_heavy";

/// The class rung `rung` of `l` drew.
fn drawn(l: &Ladder, rung: usize) -> Option<String> {
    classes::drawn_class(l.extras(rung)).map(str::to_string)
}

#[test]
fn over_many_seeds_a_shotgun_rung_draws_each_of_its_four_classes() {
    let mut sim = Sim::with(Tf2Sim, |app| {
        app.add_plugins((GreyboxMapPlugin, MinigamePlugin));
        app.add_minigames(&[("kit_shotgun_heavy.toml", DEF)]).expect("the definition reads");
    });
    let def = sim.app.world().resource::<Minigames>().get(ID).expect("the definition");
    let mut seen = BTreeSet::new();
    for seed in 0..64 {
        sim.app.insert_resource(RungSeed(seed));
        let l = ladder::draw(sim.app.world(), def).expect("a ladder");
        seen.insert(drawn(&l, 0).expect("the Shotgun's rung drew a class"));
        assert_eq!(drawn(&l, 1).as_deref(), Some("heavy"), "seed {seed}: a Heavy-only gun is the Heavy's");
        // The class shows after the gun in the HUD's rung line, only
        // where there was a choice.
        assert!(l.extras(0).unwrap().label.is_some() && l.extras(1).unwrap().label.is_none());
        // The same seed draws the same.
        assert_eq!(ladder::draw(sim.app.world(), def).unwrap().extras, l.extras, "seed {seed}");
    }
    assert_eq!(seen.into_iter().collect::<Vec<_>>(), ["engineer", "heavy", "pyro", "soldier"]);
}

/// A seed whose ladder makes the Shotgun's rung `class`'s.
fn seed_for(class: &str) -> u64 {
    let mut sim = Sim::with(Tf2Sim, |app| {
        app.add_plugins((GreyboxMapPlugin, MinigamePlugin));
        app.add_minigames(&[("kit_shotgun_heavy.toml", DEF)]).expect("the definition reads");
    });
    let def = sim.app.world().resource::<Minigames>().get(ID).expect("the definition");
    (0..256)
        .find(|s| {
            sim.app.insert_resource(RungSeed(*s));
            drawn(&ladder::draw(sim.app.world(), def).unwrap(), 0).as_deref() == Some(class)
        })
        .expect("a seed")
}

/// A server of the gun game (TF2's movement), with `clients` clients.
fn server(seed: u64, clients: usize) -> NetSim {
    let mut sim = NetSim::new(
        Tf2Sim,
        LinkConditions {
            latency: std::time::Duration::from_millis(20),
            jitter: std::time::Duration::ZERO,
            loss: 0.0,
        },
        5,
        clients,
        |app| {
            app.add_plugins((GreyboxMapPlugin, MinigamePlugin, MinigameNetPlugin))
                .insert_resource(Loadout { movement: movement::ID });
            app.world_mut().get_resource_or_init::<Games>().register(&INFO);
            app.add_minigames(&[("kit_shotgun_heavy.toml", DEF)]).expect("the definition reads");
            app.insert_resource(AssetHost::new("https://assets.invalid:1", "ab".repeat(32)));
        },
    );
    sim.set_frame(tf2::game::TICK_INTERVAL);
    sim.server.app.insert_resource(RungSeed(seed));
    minigame::start(sim.server.app.world_mut(), ID).expect("it starts");
    sim
}

/// Player `p`'s class, body, top speed (units/s) and max health (points)
/// in `w`.
fn worn(w: &World, p: Entity) -> (Option<String>, Option<String>, Option<f32>, Option<f32>) {
    let table = w.resource::<Classes>();
    (
        w.get::<Tf2Class>(p).map(|c| c.class(table).name.clone()),
        BodyName::named(w.get::<BodyName>(p)).map(|b| b.0.clone()),
        w.get::<MoveTraits>(p).and_then(|t| t.max_speed),
        w.get::<Health>(p).map(|h| (h.max * 100.0).round()),
    )
}

#[test]
fn the_drawn_class_is_shared_and_its_body_speed_and_health_follow() {
    let class = "pyro";
    let mut sim = server(seed_for(class), 1);
    // A player of the server's own, as a bot is.
    let bot = sim.server.spawn_character(chimera_world::greybox::SPAWNS[1], movement::ID);
    sim.until_joined(900);
    sim.ticks(200);
    let l = sim.server.app.world().resource::<Ladder>().clone();
    assert_eq!(l.rungs[0], SHOTGUN);
    assert_eq!(drawn(&l, 0).as_deref(), Some(class), "the rung drew the Pyro");
    let client = sim.character_of(0).expect("the client plays");
    for p in [client, bot] {
        // TF2's identity, as a party's slots give a TF2 gun's holder
        // (replicated).
        sim.server.app.world_mut().entity_mut(p).insert(GameIdentity(tf2::game::GAME.into()));
        ladder::set_rung(sim.server.app.world_mut(), p, 0, &l);
    }
    sim.ticks(120);
    let want = (Some(class.to_string()), Some("models/player/pyro.mdl".to_string()), Some(300.0), Some(175.0));
    for p in [client, bot] {
        let w = sim.server.app.world();
        assert_eq!(w.get::<RungClass>(p).map(|c| c.0.as_str()), Some(class), "{p}: the rung's class on the server");
        assert_eq!(worn(w, p), want, "{p} on the server");
    }
    // The client has the server's draw and wears it on its own player.
    let me = sim.local_player(0).expect("the client's own player");
    let w = sim.clients[0].app.world();
    assert_eq!(w.resource::<Ladder>().extras, l.extras, "the client has the server's rung extras");
    assert_eq!(w.get::<RungClass>(me).map(|c| c.0.as_str()), Some(class), "the client's own player");
    let (c, body, speed, _) = worn(w, me);
    assert_eq!((c, body, speed), (want.0.clone(), want.1.clone(), want.2), "the client's own player wears the Pyro");
    // The Minigun's rung makes the Heavy whatever was drawn before.
    ladder::set_rung(sim.server.app.world_mut(), bot, 1, &l);
    sim.ticks(60);
    let w = sim.server.app.world();
    assert_eq!(w.get::<chimera_sim::weapon::Inventory>(bot).and_then(|i| i.active).and_then(|e| w.get::<chimera_sim::weapon::Weapon>(e)).map(|g| g.id), Some(MINIGUN));
    assert_eq!(worn(w, bot), (Some("heavy".into()), Some("models/player/heavy.mdl".into()), Some(230.0), Some(300.0)));
}

/// The offer's TF2 class sets.
fn class_sets(m: &MatchManifest) -> Vec<String> {
    let offer = m.assets.as_ref().expect("an offer");
    offer
        .now
        .iter()
        .filter(|g| g.game == "tf2")
        .flat_map(|g| g.sets.clone())
        .filter(|s| s.starts_with("players/"))
        .collect()
}

#[test]
fn the_offer_names_only_the_drawn_classes() {
    for class in ["pyro", "soldier", "engineer"] {
        let mut sim = server(seed_for(class), 0);
        sim.ticks(10);
        let sets = class_sets(&manifest::current(sim.server.app.world_mut()));
        // Class order: the drawn class and the Minigun's Heavy, no other.
        let mut want = vec![format!("players/{class}"), "players/heavy".to_string()];
        let order = |s: &String| Classes::default().get(&s["players/".len()..]).unwrap().index;
        want.sort_by_key(order);
        assert_eq!(sets, want, "a Shotgun rung that drew the {class}");
    }
}
