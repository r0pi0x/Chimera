//! Guns that do no damage stay off drawn ladders (the user's decision,
//! 2026-11-12): the predicate `weapon::harm::can_damage_player` reads each
//! weapon's own data (the M136_AT4's blast is 0, the RMS12's burn),
//! `auto::drawable` leaves those out and the ones a game's data excludes
//! (Combat Arms' data/ladder_exclusions.toml: the stun launchers, the
//! skins drawn wrong), the Ultimate Gun Game's and a `ladder = "auto"`
//! draw never deal them, and a written ladder still may. The install's
//! copies are fixtures here, shaped as the install gives them (classed,
//! their numbers over their template's); the install's own:
//! combat_arms' `heavy::no_damage_install`.

use bevy::prelude::*;
use chimera_sim::{
    minigame::{self, MinigameAppExt, Minigames, auto, ladder::Ladder, power::{PowerCurve, PowerReaders}},
    weapon::{
        data::{WeaponCatalog, WeaponDataAppExt},
        harm::can_damage_player,
    },
};
use chimera_world::harness::Sim;
use ultimate_gun_game::{MINIGAME, RUNGS, generate};

use crate::parts::{rungs, ugg};

const AT4: &str = "combat_arms:m136_at4";
const RMS12: &str = "combat_arms:rms12";
/// The install's zero-blast launcher with a class (an M136_AT4 copy).
const APOLLO: &str = "combat_arms:optical_buster_apollo";
const STUN: &str = "combat_arms:infecteds_stunup";

/// The install's copies as it makes them (special_weapons.md 3, 4).
fn copies() -> [(&'static str, String); 3] {
    [
        (
            "rms12",
            format!(
                "id = \"{RMS12}\"\nbase = \"combat_arms:flamethrower\"\nclass = \"flamethrower\"\n\
                 [hooks.\"combat_arms:projectile\".over_time]\nrate = 0.0\n"
            ),
        ),
        (
            "apollo",
            format!("id = \"{APOLLO}\"\nbase = \"{AT4}\"\nclass = \"launcher\"\n"),
        ),
        (
            "stunup",
            format!(
                "id = \"{STUN}\"\nbase = \"combat_arms:m32\"\nclass = \"launcher\"\n\
                 [hooks.\"combat_arms:projectile\"]\narea_damage = 15.0\narea_radius = 400.0\n"
            ),
        ),
    ]
}

/// The Ultimate Gun Game with the install's copies in its catalogue.
fn ugg_with_copies() -> Sim {
    let mut sim = ugg();
    let docs = copies();
    let docs: Vec<(&str, &str)> = docs.iter().map(|(n, t)| (*n, t.as_str())).collect();
    sim.app.add_weapon_data(&docs).expect("the copies read");
    sim
}

fn harm(world: &World, id: &str) -> Option<bool> {
    let data = world.resource::<WeaponCatalog>().get(id).unwrap_or_else(|| panic!("{id} in the catalogue"));
    can_damage_player(data, &world.resource::<PowerReaders>().harm)
}

#[test]
fn the_predicate_reads_each_weapons_data() {
    let sim = ugg_with_copies();
    let w = sim.app.world();
    for id in [AT4, RMS12, APOLLO, "cs_source:weapon_smokegrenade"] {
        assert_eq!(harm(w, id), Some(false), "{id}");
    }
    for id in [
        "combat_arms:law",
        "combat_arms:m32",
        "combat_arms:flamethrower",
        "combat_arms:m4a1",
        "combat_arms:m67",
        "combat_arms:m9_knife",
        "cs_source:weapon_ak47",
        "cs_source:weapon_knife",
        "cs_source:weapon_hegrenade",
    ] {
        assert_eq!(harm(w, id), Some(true), "{id}");
    }
    // The stun shells blast a little (3.2): out by the data's list, not
    // by the damage test.
    assert_eq!(harm(w, STUN), Some(true));
}

#[test]
fn drawn_ladders_never_deal_a_gun_without_damage() {
    let mut sim = ugg_with_copies();
    let all = auto::report(sim.app.world());
    let mut caps = all.clone();
    let skipped = auto::drawable(sim.app.world(), &mut caps);
    let ids: Vec<&str> = skipped.iter().map(|s| s.id).collect();
    assert_eq!(ids, [RMS12, APOLLO, STUN], "{}", auto::skipped_lines(&skipped));
    assert_eq!(skipped[0].why, "no damage");
    assert!(skipped[2].why.contains("stun"), "{}", skipped[2].why);
    {
        let w = sim.app.world();
        let catalog = w.resource::<WeaponCatalog>();
        let readers = &w.resource::<PowerReaders>().harm;
        for c in &caps {
            for id in c.guns.iter().map(|g| g.id).chain(c.grenades.iter().copied()) {
                assert_ne!(can_damage_player(catalog.get(id).unwrap(), readers), Some(false), "{id} left in");
            }
        }
        // Unfiltered, the draw deals them now and then; filtered, never.
        let powers = crate::parts::powers(w, &all);
        let curve = &PowerCurve::shipped();
        let draw = |caps: &[auto::Capabilities], seed| {
            let refs: Vec<&auto::Capabilities> = caps.iter().collect();
            auto::ladder(&refs, &powers, curve, RUNGS, seed).rungs
        };
        assert!((0..500).any(|s| draw(&all, s).contains(&RMS12)), "the RMS12 drawn unfiltered");
        assert!((0..500).all(|s| !draw(&caps, s).iter().any(|id| ids.contains(id))));
    }
    // The Ultimate Gun Game's ladders, seed after seed.
    for seed in 0..60 {
        generate(sim.app.world_mut(), seed, RUNGS).expect("made");
        let l = rungs(sim.app.world());
        assert!(!l.iter().any(|id| ids.contains(&id.as_str())), "seed {seed}: {l:?}");
    }
    // A definition's `ladder = "auto"`, drawn at each start.
    let text = sim.app.world().resource::<Minigames>().text(MINIGAME).unwrap().to_string();
    let start = text.find("ladder = [").expect("a written ladder");
    let end = start + text[start..].find(']').unwrap() + 1;
    let def = |ladder: &str, id: &str| {
        format!("{}ladder = {ladder}{}", &text[..start], &text[end..]).replace(&format!("id = \"{MINIGAME}\""), &format!("id = \"{id}\""))
    };
    sim.app.add_minigames(&[("drawn.toml", &def("\"auto\"\nrungs = 30", "test:drawn"))]).unwrap();
    for _ in 0..30 {
        minigame::start(sim.app.world_mut(), "test:drawn").expect("starts");
        let l = &sim.app.world().resource::<Ladder>().rungs;
        assert!(!l.iter().any(|id| ids.contains(id)), "{l:?}");
    }
    // A written ladder may name them.
    let written = format!("[\"{AT4}\", \"{RMS12}\", \"{STUN}\", \"combat_arms:law\"]");
    sim.app.add_minigames(&[("written.toml", &def(&written, "test:written"))]).unwrap();
    minigame::start(sim.app.world_mut(), "test:written").expect("starts");
    let l = sim.app.world().resource::<Ladder>();
    assert_eq!(&l.rungs[..4], &[AT4, RMS12, STUN, "combat_arms:law"], "as written");
}
