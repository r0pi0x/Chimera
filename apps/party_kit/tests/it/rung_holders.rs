//! Characters follow the gun (`chimera_sim::minigame::holders`, each
//! game's data/rung_characters.toml; docs/DESIGN.md "Gun game rules"): on
//! the party's simulation, a CS:S rung whose gun only Terrorists buy
//! (the AK-47, `buy.team` 1) always draws a T model and puts it on the
//! players there, a CT-only gun (the M4A1) a CT model, a gun both sides
//! buy (the Desert Eagle) models of both sides over many seeds; a
//! definition's override (`rung_extras = { character = { weapons = .. }
//! }`) wins over what is derived. A Combat Arms gun draws a gun holder's
//! body (the rung bodies, never only the infected's), an override by
//! record id or by group gives those bodies only.

use bevy::prelude::*;
use chimera_core::core::PlayerClass;
use chimera_sim::{
    minigame::{
        self, MinigameAppExt,
        ladder::{self, Ladder, RungSeed},
    },
    weapon::data::WeaponCatalog,
};
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim};
use combat_arms::{
    game::{data::CaData, rungs},
    harness::CaSim,
};
use cs_source::game::rungs as css;
use party_kit::PartyPartsPlugin;

use super::gun_game::soldier;

const AK: &str = "cs_source:weapon_ak47";
const M4A1: &str = "cs_source:weapon_m4a1";
const DEAGLE: &str = "cs_source:weapon_deagle";
const CA_M4: &str = "combat_arms:m4a1";
const CA_M60: &str = "combat_arms:m60";

/// A ladder of these four guns, with `extra` in `[rules]`.
fn definition(extra: &str) -> String {
    format!(
        r#"
id = "test:holders"
name = "Holders"
players = {{ min = 1, max = 8 }}
maps = ["chimera:greybox"]
teams = "free_for_all"
time_limit = "5m"

[slots]
identity = "weapon"
weapons = ["cs_source", "combat_arms"]

[spawn]
respawn = {{ delay = "1s", protection = "0s" }}
points = "map"

[rules]
module = "chimera:ladder"
ladder = ["{AK}", "{M4A1}", "{DEAGLE}", "{CA_M4}", "{CA_M60}"]
{extra}
"#
    )
}

fn party(extra: &str, seed: u64) -> Sim {
    let mut sim = Sim::new(CaSim, (GreyboxMapPlugin, PartyPartsPlugin));
    sim.app.insert_resource(RungSeed(seed));
    sim.app.add_minigames(&[("holders.toml", &definition(extra))]).unwrap();
    minigame::start(sim.app.world_mut(), "test:holders").expect("it starts");
    sim
}

/// Rung `i`'s drawn CS:S class.
fn class(l: &Ladder, i: usize) -> u8 {
    l.extras(i).and_then(|x| x.value(css::CLASS)).expect("a CS:S class").parse().unwrap()
}

/// Rung `i`'s drawn Combat Arms body.
fn body(l: &Ladder, i: usize) -> i32 {
    l.extras(i).and_then(|x| x.value(rungs::MODEL)).expect("a Combat Arms body").parse().unwrap()
}

/// The ladders of seeds 1..=n.
fn ladders(extra: &str, n: u64) -> Vec<Ladder> {
    (1..=n).map(|seed| party(extra, seed).app.world().resource::<Ladder>().clone()).collect()
}

#[test]
fn a_css_rung_draws_a_model_of_the_side_that_buys_its_gun() {
    let all = ladders("", 24);
    let (mut deagle_t, mut deagle_ct) = (false, false);
    for l in &all {
        assert!((1..=4).contains(&class(l, 0)), "the AK-47 (T only): a T model, got {}", class(l, 0));
        assert!((5..=8).contains(&class(l, 1)), "the M4A1 (CT only): a CT model, got {}", class(l, 1));
        let d = class(l, 2);
        deagle_t |= (1..=4).contains(&d);
        deagle_ct |= (5..=8).contains(&d);
    }
    assert!(deagle_t && deagle_ct, "the Desert Eagle (both sides): either side's models");
    // Worn by the players on the rung.
    let mut sim = party("", 5);
    let p = soldier(&mut sim);
    sim.seconds(1.5);
    let l = sim.app.world().resource::<Ladder>().clone();
    ladder::set_rung(sim.app.world_mut(), p, 0, &l);
    sim.seconds(0.5);
    let worn = sim.app.world().get::<PlayerClass>(p).map(|c| c.0);
    assert_eq!(worn, Some(class(&l, 0)), "the AK-47's T model worn");
    ladder::set_rung(sim.app.world_mut(), p, 1, &l);
    sim.seconds(0.5);
    let worn = sim.app.world().get::<PlayerClass>(p).map(|c| c.0);
    assert!(worn.is_some_and(|c| (5..=8).contains(&c)), "the M4A1's CT model worn: {worn:?}");
}

#[test]
fn the_data_derives_the_css_mapping_from_the_buy_team() {
    let sim = party("", 1);
    let catalog = sim.app.world().resource::<WeaponCatalog>();
    let h = css::holders();
    assert_eq!(css::classes_for(&h, Some(catalog), AK), [1, 2, 3, 4]);
    assert_eq!(css::classes_for(&h, Some(catalog), M4A1), [5, 6, 7, 8]);
    assert_eq!(css::classes_for(&h, Some(catalog), DEAGLE), [1, 2, 3, 4, 5, 6, 7, 8]);
    // Every team-only gun of the catalogue maps to its side alone.
    let mut counts = [0usize; 3];
    for set in &catalog.sets {
        for w in set.weapons.iter().filter(|w| w.id.starts_with("cs_source:") && w.buy.is_some()) {
            let classes = css::classes_for(&h, Some(catalog), &w.id);
            match w.buy.as_ref().and_then(|b| b.team) {
                Some(1) => assert_eq!(classes, [1, 2, 3, 4], "{}", w.id),
                Some(2) => assert_eq!(classes, [5, 6, 7, 8], "{}", w.id),
                _ => assert_eq!(classes.len(), 8, "{}", w.id),
            }
            counts[match w.buy.as_ref().and_then(|b| b.team) {
                Some(1) => 0,
                Some(2) => 1,
                _ => 2,
            }] += 1;
        }
    }
    eprintln!("CS:S bought weapons: {} T only, {} CT only, {} either", counts[0], counts[1], counts[2]);
    assert!(counts[0] >= 6 && counts[1] >= 6, "{counts:?}");
}

#[test]
fn a_definition_overrides_who_holds_a_gun() {
    let extra = r#"rung_extras = { character = { weapons = { "cs_source:weapon_ak47" = ["ct"], "combat_arms:m4a1" = ["1"], "combat_arms:m60" = ["zombies"] } } }"#;
    let all = ladders(extra, 12);
    let sim = party("", 1);
    let data = sim.app.world().resource::<CaData>();
    let zombies = data.characters().infected_bodies();
    for l in &all {
        assert!((5..=8).contains(&class(l, 0)), "the AK-47 overridden to CT: {}", class(l, 0));
        assert!((5..=8).contains(&class(l, 1)), "the M4A1 still CT");
        assert_eq!(body(l, 3), 1, "the Combat Arms M4A1 overridden to record 1");
        assert!(zombies.contains(&body(l, 4)), "the M60 overridden to the zombies' bodies: {}", body(l, 4));
    }
}

#[test]
fn a_combat_arms_gun_draws_a_gun_holders_body() {
    let all = ladders("", 16);
    let sim = party("", 1);
    let w = sim.app.world();
    let players = w.resource::<CaData>().characters().rung_bodies();
    let mut seen: Vec<i32> = Vec::new();
    for l in &all {
        for i in [3, 4] {
            let b = body(l, i);
            assert!(players.contains(&b), "rung {i}: {b} is a player body ({players:?})");
            if !seen.contains(&b) {
                seen.push(b);
            }
        }
    }
    assert!(seen.len() > 1, "drawn among them: {seen:?}");
    // The mapping: a gun to every player body; an infected's weapon (none
    // here without the install) would derive the infected's.
    let catalog = w.resource::<WeaponCatalog>();
    let h = rungs::holders();
    assert_eq!(rungs::bodies_for(&h, Some(catalog), Some(w.resource::<CaData>()), CA_M4), players);
    assert_eq!(h.of("combat_arms:quarantine_virus_hand", Some("infected")), ["@infected"]);
}
