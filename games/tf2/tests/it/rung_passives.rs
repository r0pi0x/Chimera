//! TF2's rung passives (`game::passives`, data/rung_passives.toml;
//! docs/plans/active/rung-passives.md, P1) with a made-up item schema
//! through the converter (the spec's attribute values, nothing of the
//! game's): with the host's passives on, a Soldier Rocket Launcher rung
//! draws the Mantreads, a Shotgun rung never draws an item (the Soldier's
//! boots take the Shotgun's secondary slot; the other classes have none
//! here), a Sniper Rifle rung one of the Sniper's back items; off, none.
//! The item reaches a network client through the inventory, worn
//! (`Unselectable`), and leaves with the rung. A Mantreads stomp kill
//! climbs and costs the victim no rung (`demote_on = "melee_death"`).

use bevy::prelude::*;
use chimera_core::{
    core::{Health, MovementState, Velocity},
    game_registry::Games,
    slots::{GameIdentity, Loadout},
};
use chimera_net::{harness::NetSim, minigame_net::MinigameNetPlugin, net::memory::LinkConditions};
use chimera_sim::{
    minigame::{
        self, MinigameAppExt, MinigamePlugin,
        ladder::{self, Ladder, Rung, RungSeed},
    },
    weapon::{Inventory, Unselectable, Weapon},
};
use chimera_world::{
    greybox::{self, GreyboxMapPlugin},
    harness::Sim,
};
use tf2::{
    game::{
        INFO, TICK_INTERVAL,
        items::{ItemPack, convert},
        movement,
        weapons::{ROCKET_LAUNCHER, SHOTGUN, SNIPER_RIFLE, script_numbers::GunNumbers},
    },
    harness::Tf2Sim,
};

const U: f32 = 0.0254;

const DEFS: &[(&str, &str)] = &[
    ("airblast_vulnerability_multiplier", "value_is_percentage"),
    ("boots_falling_stomp", "value_is_additive"),
    ("damage_force_reduction", "value_is_percentage"),
    ("set_blockbackstab_once", "value_is_additive"),
    ("item_meter_charge_rate", "value_is_additive"),
    ("add_maxhealth", "value_is_additive"),
];

/// (index, name, class, slot, used by, attributes).
#[allow(clippy::type_complexity)]
const ITEMS: &[(u32, &str, &str, &str, &str, &[(&str, &str)])] = &[
    (444, "The Mantreads", "tf_wearable", "secondary", "soldier", &[
        ("airblast_vulnerability_multiplier", "0.25"), ("boots_falling_stomp", "1"), ("damage_force_reduction", "0.25"),
    ]),
    (57, "The Razorback", "tf_wearable_razorback", "secondary", "sniper", &[("set_blockbackstab_once", "1"), ("item_meter_charge_rate", "30")]),
    (231, "Darwin's Danger Shield", "tf_wearable", "secondary", "sniper", &[("add_maxhealth", "25")]),
    (405, "Ali Baba's Wee Booties", "tf_wearable", "secondary", "demoman", &[("add_maxhealth", "25")]),
];

fn schema() -> String {
    let mut s = String::from("\"items_game\"\n{\n\"items\"\n{\n");
    for (index, name, class, slot, used, attrs) in ITEMS {
        s += &format!("\"{index}\" {{ \"name\" \"{name}\" \"item_class\" \"{class}\" \"item_slot\" \"{slot}\" \"used_by_classes\" {{ \"{used}\" \"1\" }} \"attributes\" {{\n");
        for (a, v) in *attrs {
            s += &format!("\"{a}\" {{ \"value\" \"{v}\" }}\n");
        }
        s += "} }\n";
    }
    s += "}\n\"attributes\"\n{\n";
    for (i, (class, format)) in DEFS.iter().enumerate() {
        s += &format!("\"{i}\" {{ \"name\" \"{class}\" \"attribute_class\" \"{class}\" \"description_format\" \"{format}\" }}\n");
    }
    s + "}\n}\n"
}

fn pack() -> ItemPack {
    let p = convert::convert(&schema(), &|_| None).pack;
    ItemPack::parse(&p.to_text()).unwrap()
}

#[derive(Clone)]
struct WithItems;

impl Plugin for WithItems {
    fn build(&self, app: &mut App) {
        app.insert_resource(GunNumbers::builtin().with_pack(&pack()));
        Plugin::build(&Tf2Sim, app);
    }
}

const ID: &str = "kit:tf2_passives";

fn def(rules: &str) -> String {
    format!(
        r#"
id = "{ID}"
name = "TF2 passives"
players = {{ min = 2, max = 4 }}
maps = ["chimera:greybox"]
teams = "free_for_all"
time_limit = "60s"
rotate = false

[spawn]
respawn = {{ delay = "1s", protection = "0.5s" }}
points = "map"

[rules]
module = "chimera:ladder"
ladder = ["{ROCKET_LAUNCHER}", "{SHOTGUN}", "{SNIPER_RIFLE}"]
{rules}
"#
    )
}

const ON: &str = "demote_on = \"melee_death\"\nrung_extras = { passives = { odds = 1 } }";

fn sim_with(rules: &str, seed: u64) -> Sim {
    let text = def(rules);
    let mut sim = Sim::with(WithItems, |app| {
        app.add_plugins((GreyboxMapPlugin, MinigamePlugin)).insert_resource(Loadout { movement: movement::ID });
        app.add_minigames(&[("kit_tf2_passives.toml", Box::leak(text.into_boxed_str()) as &str)]).expect("the definition reads");
        app.insert_resource(RungSeed(seed));
    });
    sim.set_tick_interval(TICK_INTERVAL);
    minigame::start(sim.app.world_mut(), ID).expect("it starts");
    sim
}

fn passives(l: &Ladder, rung: usize) -> Vec<String> {
    l.extras(rung).map(|x| x.passives.clone()).unwrap_or_default()
}

#[test]
fn a_rocket_rung_draws_the_mantreads_a_shotgun_rung_never_and_off_draws_none() {
    for seed in 0..24 {
        let l = sim_with(ON, seed).app.world().resource::<Ladder>().clone();
        assert_eq!(passives(&l, 0), ["tf2:mantreads"], "seed {seed}: the Soldier's only item, at odds 1");
        assert!(passives(&l, 1).is_empty(), "seed {seed}: the Shotgun ({:?}) takes the secondary slot", l.extras(1));
        let back = passives(&l, 2);
        assert!(back.len() == 1 && ["tf2:razorback", "tf2:darwins_danger_shield"].contains(&back[0].as_str()), "seed {seed}: {back:?}");
        let off = sim_with("", seed).app.world().resource::<Ladder>().clone();
        assert!((0..3).all(|r| passives(&off, r).is_empty()), "seed {seed}: off by default");
    }
}

fn worn(w: &World, p: Entity) -> Vec<(&'static str, bool)> {
    let Some(inv) = w.get::<Inventory>(p) else { return Vec::new() };
    inv.weapons
        .iter()
        .filter_map(|e| w.get::<Weapon>(*e).map(|x| (x.id, w.get::<Unselectable>(*e).is_some())))
        .filter(|(id, _)| ["tf2:mantreads", "tf2:razorback", "tf2:darwins_danger_shield"].contains(id))
        .collect()
}

#[test]
fn a_client_gets_the_worn_item_through_the_inventory_and_loses_it_with_the_rung() {
    let text: &'static str = Box::leak(def(ON).into_boxed_str());
    let mut sim = NetSim::new(
        WithItems,
        LinkConditions {
            latency: std::time::Duration::from_millis(20),
            jitter: std::time::Duration::ZERO,
            loss: 0.0,
        },
        5,
        1,
        move |app| {
            app.add_plugins((GreyboxMapPlugin, MinigamePlugin, MinigameNetPlugin)).insert_resource(Loadout { movement: movement::ID });
            app.world_mut().get_resource_or_init::<Games>().register(&INFO);
            app.add_minigames(&[("kit_tf2_passives.toml", text)]).expect("the definition reads");
        },
    );
    sim.set_frame(TICK_INTERVAL);
    sim.server.app.insert_resource(RungSeed(3));
    minigame::start(sim.server.app.world_mut(), ID).expect("it starts");
    let _bot = sim.server.spawn_character(greybox::SPAWNS[1], movement::ID);
    sim.until_joined(900);
    sim.ticks(200);
    let l = sim.server.app.world().resource::<Ladder>().clone();
    let client = sim.character_of(0).expect("the client plays");
    sim.server.app.world_mut().entity_mut(client).insert(GameIdentity(tf2::game::GAME.into()));
    ladder::set_rung(sim.server.app.world_mut(), client, 0, &l);
    sim.ticks(120);
    assert_eq!(worn(sim.server.app.world(), client), [("tf2:mantreads", true)], "worn on the server, never drawn");
    let me = sim.local_player(0).expect("the client's own player");
    assert_eq!(sim.clients[0].app.world().resource::<Ladder>().extras, l.extras, "the client has the server's passives");
    assert_eq!(worn(sim.clients[0].app.world(), me), [("tf2:mantreads", true)], "the client's inventory has it, worn");
    // The Shotgun's rung has none: the boots come off on both.
    ladder::set_rung(sim.server.app.world_mut(), client, 1, &l);
    sim.ticks(120);
    assert!(worn(sim.server.app.world(), client).is_empty());
    assert!(worn(sim.clients[0].app.world(), me).is_empty(), "taken on the client too");
}

fn rung(sim: &Sim, p: Entity) -> usize {
    sim.app.world().get::<Rung>(p).map_or(0, |r| r.0)
}

#[test]
fn a_mantreads_stomp_kill_climbs_and_never_demotes() {
    let mut sim = sim_with(ON, 1);
    let a = sim.spawn_character(greybox::SPAWNS[0], movement::ID);
    let b = sim.spawn_character(greybox::SPAWNS[1], movement::ID);
    sim.seconds(1.7);
    // b up a rung first, so a demotion would show.
    let l = sim.app.world().resource::<Ladder>().clone();
    ladder::set_rung(sim.app.world_mut(), b, 1, &l);
    sim.seconds(1.0);
    assert_eq!((rung(&sim, a), rung(&sim, b)), (0, 1));
    assert_eq!(worn(sim.app.world(), a), [("tf2:mantreads", true)]);
    // a lands on b's head falling at 900 (as batch3b's stomp test).
    sim.app.world_mut().get_mut::<Health>(b).unwrap().current = 0.3;
    let top = sim.position(b) + Vec3::Y * 90.0 * U;
    sim.app.world_mut().get_mut::<Transform>(a).unwrap().translation = top + Vec3::Y * 40.0 * U;
    sim.app.world_mut().get_mut::<MovementState>(a).unwrap().on_ground = false;
    for _ in 0..12 {
        if !sim.app.world().get::<MovementState>(a).unwrap().on_ground {
            sim.app.world_mut().get_mut::<Velocity>(a).unwrap().0 = Vec3::NEG_Y * 888.0 * U;
        }
        sim.ticks(1);
    }
    sim.seconds(0.5);
    assert_eq!((rung(&sim, a), rung(&sim, b)), (1, 1), "the stomp's kill climbs; the victim keeps his rung");
}
