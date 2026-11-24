//! TF2's rung passives (`game::passives`, data/rung_passives.toml;
//! docs/plans/active/rung-passives.md, P1) with a made-up item schema
//! through the converter (the spec's attribute values, nothing of the
//! game's): with the host's passives on, a Soldier Rocket Launcher rung
//! draws the Mantreads or the B.A.S.E. Jumper, a Shotgun rung never draws an item (the Soldier's
//! boots take the Shotgun's secondary slot; the other classes have none
//! here), a Sniper Rifle rung one of the Sniper's back items; off, none.
//! P3's keyed items: a shield or the chute with the Grenade Launcher, a
//! watch with the Revolver, the Thermal Thruster with the Flame Thrower,
//! none with the Stickybomb Launcher or the Shotgun (slot and attack2);
//! each works with the rung's gun out (attack2, jump in the air, the
//! thruster's slot key); bots use them (`items::bots`, a stand-in brain
//! writing `BotFocus`); leaving the rung mid-charge or mid-cloak ends it
//! on the server that tick and on the client from that tick (`NetSim`).
//! The item reaches a network client through the inventory, worn
//! (`Unselectable`), and leaves with the rung. A Mantreads stomp kill
//! climbs and costs the victim no rung (`demote_on = "melee_death"`).

use bevy::prelude::*;
use chimera_core::{
    core::{Bot, BotFocus, Health, Intent, MovementState, Velocity},
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
        items::{ItemPack, convert, kit::{Tf2Chute, Tf2Thruster}, shields::Tf2Shield, spy::Tf2Cloak},
        movement,
        weapons::{FLAMETHROWER, GRENADE_LAUNCHER, REVOLVER, ROCKET_LAUNCHER, SHOTGUN, SNIPER_RIFLE, STICKY_LAUNCHER, script_numbers::GunNumbers},
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
    ("mult_dmgtaken_from_explosions", "value_is_percentage"),
    ("charge_recharge_rate", "value_is_percentage"),
    ("charge_turn_control", "value_is_percentage"),
    ("parachute_attribute", "value_is_additive"),
    ("thermal_thruster", "value_is_additive"),
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
    // P3's keyed and carried items.
    (131, "The Chargin' Targe", "tf_wearable_demoshield", "secondary", "demoman", &[("mult_dmgtaken_from_explosions", "0.7")]),
    (406, "The Splendid Screen", "tf_wearable_demoshield", "secondary", "demoman", &[("charge_recharge_rate", "1.5")]),
    (1099, "The Tide Turner", "tf_wearable_demoshield", "secondary", "demoman", &[("charge_turn_control", "50")]),
    (30, "Invisibility Watch", "tf_weapon_invis", "pda2", "spy", &[]),
    (60, "The Cloak and Dagger", "tf_weapon_invis", "pda2", "spy", &[]),
    (59, "The Dead Ringer", "tf_weapon_invis", "pda2", "spy", &[]),
    (1101, "The B.A.S.E. Jumper", "tf_weapon_parachute", "secondary", "soldier", &[("parachute_attribute", "1")]),
    (1179, "The Thermal Thruster", "tf_weapon_rocketpack", "secondary", "pyro", &[("thermal_thruster", "1"), ("item_meter_charge_rate", "30")]),
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
    def_on(&[ROCKET_LAUNCHER, SHOTGUN, SNIPER_RIFLE], rules)
}

fn def_on(guns: &[&str], rules: &str) -> String {
    let ladder = guns.iter().map(|g| format!("\"{g}\"")).collect::<Vec<_>>().join(", ");
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
ladder = [{ladder}]
{rules}
"#
    )
}

const ON: &str = "demote_on = \"melee_death\"\nrung_extras = { passives = { odds = 1 } }";

fn sim_with(rules: &str, seed: u64) -> Sim {
    sim_on(def(rules), seed)
}

fn sim_on(text: String, seed: u64) -> Sim {
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
        let soldier = passives(&l, 0);
        assert!(
            soldier.len() == 1 && ["tf2:mantreads", "tf2:base_jumper"].contains(&soldier[0].as_str()),
            "seed {seed}: one of the Soldier's secondary items, at odds 1: {soldier:?}"
        );
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
    // A seed whose Rocket Launcher rung draws the Mantreads (not the chute).
    let seed = (0..64).find(|s| passives(sim_with(ON, *s).app.world().resource::<Ladder>(), 0) == ["tf2:mantreads"]).unwrap();
    let mut sim = sim_with(ON, seed);
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

// ---- P3: keyed and carried items ----

const SHIELDS: [&str; 3] = ["tf2:chargin_targe", "tf2:splendid_screen", "tf2:tide_turner"];
const WATCHES: [&str; 3] = ["tf2:invis_watch", "tf2:cloak_and_dagger", "tf2:dead_ringer"];
const DEMO_ITEMS: [&str; 6] =
    ["tf2:chargin_targe", "tf2:splendid_screen", "tf2:tide_turner", "tf2:ali_babas_wee_booties", "tf2:base_jumper", "tf2:bootlegger"];

#[test]
fn keyed_items_go_only_where_their_slot_and_key_are_free() {
    let guns = [GRENADE_LAUNCHER, STICKY_LAUNCHER, REVOLVER, FLAMETHROWER, SHOTGUN];
    let mut shields = 0;
    for seed in 0..24 {
        let l = sim_on(def_on(&guns, ON), seed).app.world().resource::<Ladder>().clone();
        let demo = passives(&l, 0);
        assert!(demo.len() == 1 && DEMO_ITEMS.contains(&demo[0].as_str()), "seed {seed}: Grenade Launcher {demo:?}");
        shields += SHIELDS.contains(&demo[0].as_str()) as u32;
        assert!(passives(&l, 1).is_empty(), "seed {seed}: the Stickybomb Launcher takes the secondary slot and attack2");
        let spy = passives(&l, 2);
        assert!(spy.len() == 1 && WATCHES.contains(&spy[0].as_str()), "seed {seed}: the Revolver leaves attack2 free: {spy:?}");
        assert_eq!(passives(&l, 3), ["tf2:thermal_thruster"], "seed {seed}: the Pyro's carried item with the Flame Thrower");
        assert!(passives(&l, 4).is_empty(), "seed {seed}: the Shotgun's slot is every keyed item's: {:?}", passives(&l, 4));
    }
    assert!(shields > 0, "a Grenade Launcher rung draws a shield on some seeds");
    // The key rule by itself: a Demoman's primary using attack2 gets no
    // shield, nor a Spy's gun using it a watch.
    let table = tf2::game::passives::Tf2Passives::default().0;
    let keys = vec!["attack2".to_string()];
    let all = |_: &str| true;
    let ids = |class: &str| table.candidates(class, "primary", &keys, &all).iter().map(|p| p.item.clone()).collect::<Vec<_>>();
    assert!(ids("demoman").iter().all(|i| !SHIELDS.contains(&i.as_str())), "{:?}", ids("demoman"));
    assert!(ids("demoman").iter().any(|i| i == "tf2:base_jumper"), "jump is no gun's key");
    assert!(ids("spy").is_empty(), "{:?}", ids("spy"));
}

/// A match on a ladder of `guns` whose first rung carries `item` (the
/// first seed drawing it), with two players on it; the first.
fn on_rung_with(guns: &[&str], item: &str) -> (Sim, Entity, Entity) {
    let seed = (0..64)
        .find(|s| passives(sim_on(def_on(guns, ON), *s).app.world().resource::<Ladder>(), 0) == [item])
        .unwrap_or_else(|| panic!("no seed draws {item}"));
    let mut sim = sim_on(def_on(guns, ON), seed);
    let a = sim.spawn_character(greybox::SPAWNS[0], movement::ID);
    let b = sim.spawn_character(greybox::SPAWNS[1], movement::ID);
    sim.seconds(1.7);
    assert!(holds(&sim, a, item), "{item} given on the rung");
    (sim, a, b)
}

fn item_of(sim: &Sim, p: Entity, id: &str) -> Option<Entity> {
    let w = sim.app.world();
    w.get::<Inventory>(p)?.weapons.iter().copied().find(|e| w.get::<Weapon>(*e).is_some_and(|x| x.id == id))
}

fn holds(sim: &Sim, p: Entity, id: &str) -> bool {
    item_of(sim, p, id).is_some()
}

fn active_id(sim: &Sim, p: Entity) -> Option<&'static str> {
    let w = sim.app.world();
    w.get::<Inventory>(p)?.active.and_then(|a| w.get::<Weapon>(a)).map(|x| x.id)
}

fn press(sim: &mut Sim, p: Entity, ticks: u64, key: impl Fn(&mut Intent, u64)) {
    for t in 0..ticks {
        key(&mut sim.intent(p), t);
        sim.ticks(1);
    }
}

#[test]
fn a_shield_charges_and_a_watch_cloaks_with_the_rungs_gun_out() {
    let (mut sim, p, _) = on_rung_with(&[GRENADE_LAUNCHER, SHOTGUN], "tf2:chargin_targe");
    assert_eq!(active_id(&sim, p), Some(GRENADE_LAUNCHER));
    let shield = item_of(&sim, p, "tf2:chargin_targe").unwrap();
    press(&mut sim, p, 3, |i, t| i.secondary = t == 0);
    assert!(sim.app.world().get::<Tf2Shield>(shield).unwrap().state.charging, "attack2 with the Grenade Launcher out charges");
    assert_eq!(active_id(&sim, p), Some(GRENADE_LAUNCHER), "the gun stays out");

    let (mut sim, p, _) = on_rung_with(&[REVOLVER, SHOTGUN], "tf2:invis_watch");
    assert_eq!(active_id(&sim, p), Some(REVOLVER));
    let watch = item_of(&sim, p, "tf2:invis_watch").unwrap();
    press(&mut sim, p, 3, |i, t| i.secondary = t == 0);
    assert!(sim.app.world().get::<Tf2Cloak>(watch).unwrap().state.cloaked, "attack2 with the Revolver out cloaks");
    assert_eq!(active_id(&sim, p), Some(REVOLVER));
}

#[test]
fn the_chute_opens_on_jump_and_the_thruster_launches_from_its_slot() {
    let (mut sim, p, _) = on_rung_with(&[ROCKET_LAUNCHER, SHOTGUN], "tf2:base_jumper");
    assert_eq!(active_id(&sim, p), Some(ROCKET_LAUNCHER));
    let chute = item_of(&sim, p, "tf2:base_jumper").unwrap();
    let high = sim.position(p) + Vec3::Y * 600.0 * U;
    sim.app.world_mut().get_mut::<Transform>(p).unwrap().translation = high;
    sim.ticks(10);
    press(&mut sim, p, 2, |i, t| i.jump = t == 0);
    assert!(sim.app.world().get::<Tf2Chute>(chute).unwrap().state.open, "jump in the air opens it, the Rocket Launcher out");

    let (mut sim, p, _) = on_rung_with(&[FLAMETHROWER, SHOTGUN], "tf2:thermal_thruster");
    assert_eq!(active_id(&sim, p), Some(FLAMETHROWER), "a carried item doesn't take the hand on arrival");
    let pack = item_of(&sim, p, "tf2:thermal_thruster").unwrap();
    press(&mut sim, p, 2, |i, t| i.select = (t == 0).then_some(1));
    sim.seconds(1.0);
    assert_eq!(active_id(&sim, p), Some("tf2:thermal_thruster"), "its slot's key draws it");
    press(&mut sim, p, 2, |i, t| i.fire = t == 0);
    let st = sim.app.world().get::<Tf2Thruster>(pack).unwrap().state;
    assert!(st.launch_at >= 0.0 && (st.meter - 50.0).abs() < 0.5, "fired: a launch due, a charge spent ({st:?})");
}

/// Ticks with the keys a brain would write each tick cleared first, then
/// `brain`'s.
fn as_bot(sim: &mut Sim, p: Entity, ticks: u64, mut brain: impl FnMut(&mut Sim)) {
    for _ in 0..ticks {
        {
            let mut i = sim.intent(p);
            i.secondary = false;
            i.jump = false;
            i.fire = false;
            i.select = None;
            i.move_axis = Vec2::ZERO;
        }
        brain(sim);
        sim.ticks(1);
    }
}

/// `p` a bot whose brain has `target` in sight, `units` ahead of it.
fn bot_facing(sim: &mut Sim, p: Entity, target: Entity, units: f32) {
    let at = sim.position(p);
    sim.app.world_mut().get_mut::<Transform>(target).unwrap().translation = at + Vec3::NEG_Z * units * U;
    sim.app.world_mut().entity_mut(p).insert(Bot);
    *sim.app.world_mut().get_mut::<BotFocus>(p).unwrap() = BotFocus { target: Some(target), sees: true };
    let mut i = sim.intent(p);
    i.yaw = 0.0;
    i.pitch = 0.0;
}

#[test]
fn bots_charge_cloak_open_the_chute_and_thrust() {
    // The shield: a target 300 units ahead in sight: it charges.
    let (mut sim, p, t) = on_rung_with(&[GRENADE_LAUNCHER, SHOTGUN], "tf2:chargin_targe");
    let shield = item_of(&sim, p, "tf2:chargin_targe").unwrap();
    bot_facing(&mut sim, p, t, 300.0);
    let mut charged = false;
    as_bot(&mut sim, p, 6, |s| charged |= s.app.world().get::<Tf2Shield>(shield).unwrap().state.charging);
    assert!(charged, "the bot charged at its target");

    // The watch: hurt with an enemy about: it cloaks and backs away; with
    // nobody in sight for 3 s it decloaks.
    let (mut sim, p, t) = on_rung_with(&[REVOLVER, SHOTGUN], "tf2:invis_watch");
    let watch = item_of(&sim, p, "tf2:invis_watch").unwrap();
    bot_facing(&mut sim, p, t, 400.0);
    as_bot(&mut sim, p, 3, |_| {});
    assert!(!sim.app.world().get::<Tf2Cloak>(watch).unwrap().state.cloaked, "unhurt: no cloak");
    sim.app.world_mut().get_mut::<Health>(p).unwrap().current *= 0.3;
    as_bot(&mut sim, p, 3, |_| {});
    assert!(sim.app.world().get::<Tf2Cloak>(watch).unwrap().state.cloaked, "hurt: cloaked");
    let before = sim.position(p).z;
    as_bot(&mut sim, p, 20, |_| {});
    assert!(sim.position(p).z > before + 10.0 * U, "it backs away from its target");
    *sim.app.world_mut().get_mut::<BotFocus>(p).unwrap() = BotFocus::default();
    as_bot(&mut sim, p, (3.5 / TICK_INTERVAL) as u64, |_| {});
    assert!(!sim.app.world().get::<Tf2Cloak>(watch).unwrap().state.cloaked, "decloaked with nobody about");

    // The chute: a long fall opens it.
    let (mut sim, p, t) = on_rung_with(&[ROCKET_LAUNCHER, SHOTGUN], "tf2:base_jumper");
    let chute = item_of(&sim, p, "tf2:base_jumper").unwrap();
    bot_facing(&mut sim, p, t, 400.0);
    let high = sim.position(p) + Vec3::Y * 1200.0 * U;
    sim.app.world_mut().get_mut::<Transform>(p).unwrap().translation = high;
    as_bot(&mut sim, p, 10, |_| {});
    assert!(!sim.app.world().get::<Tf2Chute>(chute).unwrap().state.deployed, "not at the fall's start");
    as_bot(&mut sim, p, 60, |_| {});
    assert!(sim.app.world().get::<Tf2Chute>(chute).unwrap().state.deployed, "opened on the long fall");

    // The thruster: a far target: it draws it, launches, draws its gun back.
    let (mut sim, p, t) = on_rung_with(&[FLAMETHROWER, SHOTGUN], "tf2:thermal_thruster");
    let pack = item_of(&sim, p, "tf2:thermal_thruster").unwrap();
    bot_facing(&mut sim, p, t, 1500.0);
    let mut launched = false;
    as_bot(&mut sim, p, (3.0 / TICK_INTERVAL) as u64, |s| launched |= s.app.world().get::<Tf2Thruster>(pack).unwrap().state.flying);
    assert!(launched, "the bot launched toward its far target");
    assert_eq!(active_id(&sim, p), Some(FLAMETHROWER), "and drew its gun back");
}

/// The server's and the client's top speed for the client's player.
fn tops(sim: &NetSim, theirs: Entity, me: Entity) -> (f32, f32) {
    let top = |w: &World, e: Entity| w.get::<source_engine::source_movement::SourceTraits>(e).and_then(|t| t.max_speed).unwrap_or(0.0);
    (top(sim.server.app.world(), theirs), top(sim.clients[0].app.world(), me))
}

fn charging(w: &World, p: Entity) -> bool {
    w.get::<Inventory>(p).is_some_and(|i| i.weapons.iter().any(|e| w.get::<Tf2Shield>(*e).is_some_and(|s| s.state.charging)))
}

fn cloaked(w: &World, p: Entity) -> bool {
    w.get::<Inventory>(p).is_some_and(|i| i.weapons.iter().any(|e| w.get::<Tf2Cloak>(*e).is_some_and(|s| s.state.cloaked)))
}

fn hidden(w: &World, p: Entity) -> u8 {
    w.get::<chimera_core::guise::Guise>(p).map_or(0, |g| g.hidden)
}

#[test]
fn leaving_the_rung_mid_charge_or_mid_cloak_ends_it_on_server_and_client() {
    let guns = [GRENADE_LAUNCHER, REVOLVER, SHOTGUN];
    let seed = (0..64)
        .find(|s| SHIELDS.contains(&passives(sim_on(def_on(&guns, ON), *s).app.world().resource::<Ladder>(), 0).first().map_or("", |x| x.as_str())))
        .expect("a seed with a shield on the first rung");
    let text: &'static str = Box::leak(def_on(&guns, ON).into_boxed_str());
    let mut sim = NetSim::new(
        WithItems,
        LinkConditions { latency: std::time::Duration::from_millis(20), jitter: std::time::Duration::ZERO, loss: 0.0 },
        5,
        1,
        move |app| {
            app.add_plugins((GreyboxMapPlugin, MinigamePlugin, MinigameNetPlugin)).insert_resource(Loadout { movement: movement::ID });
            app.world_mut().get_resource_or_init::<Games>().register(&INFO);
            app.add_minigames(&[("kit_tf2_passives.toml", text)]).expect("the definition reads");
        },
    );
    sim.set_frame(TICK_INTERVAL);
    sim.server.app.insert_resource(RungSeed(seed));
    minigame::start(sim.server.app.world_mut(), ID).expect("it starts");
    let _bot = sim.server.spawn_character(greybox::SPAWNS[1], movement::ID);
    sim.until_joined(900);
    sim.ticks(200);
    let l = sim.server.app.world().resource::<Ladder>().clone();
    let theirs = sim.character_of(0).expect("the client plays");
    sim.server.app.world_mut().entity_mut(theirs).insert(GameIdentity(tf2::game::GAME.into()));
    ladder::set_rung(sim.server.app.world_mut(), theirs, 0, &l);
    sim.ticks(120);
    let me = sim.local_player(0).expect("the client's own player");
    let errors = |sim: &NetSim| sim.clients[0].app.world().resource::<chimera_net::net::predict::NetGraph>().errors;

    // The client charges (predicted): 750 on both.
    for t in 0..8 {
        sim.clients[0].app.world_mut().get_mut::<Intent>(me).unwrap().secondary = t == 0;
        sim.step();
    }
    assert!(charging(sim.server.app.world(), theirs) && charging(sim.clients[0].app.world(), me), "charging on both");
    assert_eq!(tops(&sim, theirs, me), (750.0, 750.0));
    // Off the rung mid-charge: the server's ends that tick ...
    ladder::set_rung(sim.server.app.world_mut(), theirs, 2, &l);
    sim.step();
    assert!(!charging(sim.server.app.world(), theirs), "ended on the server at once");
    assert!(tops(&sim, theirs, me).0 < 750.0);
    // ... and the client's history from the server's tick has none: one
    // correction at most for the change it couldn't foresee, then the
    // same top speed as the server's and no more corrections.
    sim.ticks(20);
    let after = errors(&sim);
    assert!(!charging(sim.clients[0].app.world(), me), "ended on the client");
    for _ in 0..60 {
        sim.step();
        let (server, client) = tops(&sim, theirs, me);
        assert_eq!(server, client, "the same top speed after the rung");
    }
    assert_eq!(errors(&sim), after, "no correction once the client has the change");

    // The Revolver's rung: cloaked, then off it.
    ladder::set_rung(sim.server.app.world_mut(), theirs, 1, &l);
    sim.ticks(120);
    assert!(passives(&l, 1).iter().any(|i| WATCHES.contains(&i.as_str())));
    for t in 0..80 {
        sim.clients[0].app.world_mut().get_mut::<Intent>(me).unwrap().secondary = t == 0;
        sim.step();
    }
    assert!(cloaked(sim.server.app.world(), theirs) && cloaked(sim.clients[0].app.world(), me), "cloaked on both");
    assert!(hidden(sim.server.app.world(), theirs) > 0);
    ladder::set_rung(sim.server.app.world_mut(), theirs, 2, &l);
    sim.step();
    assert!(!cloaked(sim.server.app.world(), theirs), "ended on the server at once");
    assert_eq!(hidden(sim.server.app.world(), theirs), 0, "seen again the same tick");
    sim.ticks(20);
    assert!(!cloaked(sim.clients[0].app.world(), me), "ended on the client");
    assert_eq!(hidden(sim.clients[0].app.world(), me), 0, "the client's copy shows him");
}
