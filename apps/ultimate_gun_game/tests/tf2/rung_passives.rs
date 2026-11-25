//! The shipped defaults give rung passives in real matches (2026-11-25: the
//! archetype work saw none in the every-game session test, whose sims have
//! no TF2 item pack, and every passive is an item of the pack). With a
//! made-up item pack (the spec's attribute names, nothing of the game's)
//! and `ultimate_gun_game:auto` as shipped, nothing set: over many seeded
//! ladders about a fifth of the TF2 rungs that can carry one draw one
//! (rung_passives.toml: odds 0.2, minus the rungs whose gun takes the
//! item's slot), and a player on such a rung wears it (`RungPassive`).

use bevy::prelude::*;
use chimera_sim::minigame::{
    self,
    ladder::{Ladder, set_rung},
    passives::RungPassive,
};
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim};
use combat_arms::harness::CaSim;
use party_roster::PartyPartsPlugin;
use tf2::game::{
    items::{ItemPack, convert},
    weapons::script_numbers::GunNumbers,
};
use ultimate_gun_game::{MINIGAME, RUNGS, UltimateGunGamePlugin, generate};

const DEFS: &[(&str, &str)] = &[
    ("airblast_vulnerability_multiplier", "value_is_percentage"),
    ("boots_falling_stomp", "value_is_additive"),
    ("damage_force_reduction", "value_is_percentage"),
    ("set_blockbackstab_once", "value_is_additive"),
    ("item_meter_charge_rate", "value_is_additive"),
    ("add_maxhealth", "value_is_additive"),
    ("parachute_attribute", "value_is_additive"),
];

/// (index, name, class, slot, used by, attributes): the Soldier's and the
/// Sniper's worn items, the Demoman's boots.
#[allow(clippy::type_complexity)]
const ITEMS: &[(u32, &str, &str, &str, &str, &[(&str, &str)])] = &[
    (444, "The Mantreads", "tf_wearable", "secondary", "soldier", &[
        ("airblast_vulnerability_multiplier", "0.25"), ("boots_falling_stomp", "1"), ("damage_force_reduction", "0.25"),
    ]),
    (1101, "The B.A.S.E. Jumper", "tf_weapon_parachute", "secondary", "soldier", &[("parachute_attribute", "1")]),
    (57, "The Razorback", "tf_wearable_razorback", "secondary", "sniper", &[("set_blockbackstab_once", "1"), ("item_meter_charge_rate", "30")]),
    (231, "Darwin's Danger Shield", "tf_wearable", "secondary", "sniper", &[("add_maxhealth", "25")]),
    (405, "Ali Baba's Wee Booties", "tf_wearable", "secondary", "demoman", &[("add_maxhealth", "25")]),
];

fn pack() -> ItemPack {
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
    s += "}\n}\n";
    let p = convert::convert(&s, &|_| None).pack;
    ItemPack::parse(&p.to_text()).unwrap()
}

/// The party's automatic gun game, every game composed and counted as
/// installed, with the made-up TF2 item pack.
fn session() -> Sim {
    let games: Vec<&str> = party_roster::GAME_PARTS.iter().map(|p| p.game).collect();
    let mut sim = Sim::with(CaSim, |app| {
        app.insert_resource(GunNumbers::builtin().with_pack(&pack()));
        app.add_plugins((GreyboxMapPlugin, PartyPartsPlugin, UltimateGunGamePlugin::default()));
    });
    sim.app.world_mut().insert_resource(chimera_net::net::manifest::LocalMounts(chimera_net::net::manifest::MountReport {
        games: games
            .iter()
            .map(|g| chimera_net::net::manifest::GameMount { game: g.to_string(), installed: true, ..Default::default() })
            .collect(),
    }));
    sim.repeatable();
    sim
}

#[test]
fn the_shipped_defaults_draw_and_give_passives_over_seeded_ladders() {
    let mut sim = session();
    let (mut tf2_rungs, mut drawn, mut given, mut tried) = (0usize, 0usize, 0usize, 0usize);
    for seed in 0..40u64 {
        let w = sim.app.world_mut();
        generate(w, seed, RUNGS).expect("a ladder");
        chimera_core::gameplay::new_game(w);
        minigame::start(w, MINIGAME).expect("the match starts");
        let ladder = w.resource::<Ladder>().clone();
        let with: Vec<usize> = (0..ladder.rungs.len())
            .filter(|r| ladder.rungs[*r].starts_with("tf2:"))
            .inspect(|_| tf2_rungs += 1)
            .filter(|r| ladder.extras(*r).is_some_and(|x| x.passives.iter().any(|p| p.starts_with("tf2:"))))
            .collect();
        drawn += with.len();
        // A player armed for a drawn rung wears it (a few seeds).
        if seed < 12 {
            let p = sim.spawn_character(chimera_world::greybox::SPAWNS[0], combat_arms::game::movement::ID);
            for r in with {
                tried += 1;
                // Right after arming: the match isn't played, and its own
                // life's arming would put the player back on its rung.
                set_rung(sim.app.world_mut(), p, r, &ladder);
                let w = sim.app.world_mut();
                let n = w.query_filtered::<Entity, With<RungPassive>>().iter(w).count();
                if n > 0 {
                    given += 1;
                }
            }
            sim.app.world_mut().despawn(p);
        }
    }
    eprintln!("{tf2_rungs} TF2 rungs, {drawn} draw a passive, {given} of {tried} tried gave one");
    assert!(tf2_rungs > 0, "the ladders deal TF2 rungs");
    assert!(drawn > 0, "{tf2_rungs} TF2 rungs over 40 seeds on the shipped defaults, none drew a passive");
    assert!(given > 0, "{drawn} rungs drew a passive, {tried} tried, none was given (no `RungPassive`)");
}
