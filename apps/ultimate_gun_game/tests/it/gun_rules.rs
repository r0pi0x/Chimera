//! The host's Guns rules on the real games' guns (docs/plans/active/
//! guns-picker-v2.md, step 2; `chimera_sim::minigame::gun_rules`): the
//! cvars set on the console decide the next ladder `ugg_new` makes, and
//! `ugg_rungs` its length within the curve's bounds.

use chimera_core::console::Console;
use chimera_sim::minigame::gun_rules::GunRulesNote;

use crate::parts::{rungs, ugg};

/// `mp_guns_games_off` with every lending game but `keep` (from the parts
/// registry, so a new game is off too).
fn all_off_but(sim: &chimera_world::harness::Sim, keep: &str) -> String {
    let games: Vec<&str> =
        chimera_sim::minigame::auto::report(sim.app.world()).iter().map(|c| c.game).filter(|g| *g != keep).collect();
    format!("mp_guns_games_off \"{}\"", games.join(" "))
}

fn run(sim: &mut chimera_world::harness::Sim, lines: &[&str]) {
    for l in lines {
        sim.app.world_mut().resource_mut::<Console>().submit(*l);
        sim.ticks(1);
    }
}

#[test]
fn games_off_a_ban_and_the_rungs_shape_the_next_ladder() {
    let mut sim = ugg();
    let off = all_off_but(&sim, "cs_source");
    run(&mut sim, &[&off, "mp_guns_ban cs_source:weapon_awp", "ugg_rungs 9", "ugg_new 7"]);
    for seed in 0..12u64 {
        run(&mut sim, &[&format!("ugg_new {seed}")]);
        let r = rungs(sim.app.world());
        assert!(r.iter().all(|id| id.starts_with("cs_source:")), "only CS:S guns: {r:?}");
        assert!(!r.iter().any(|id| id == "cs_source:weapon_awp"), "the AWP is banned: {r:?}");
        let melee = sim.app.world().resource::<chimera_sim::minigame::Minigames>().get(ultimate_gun_game::MINIGAME).unwrap().rules.params["melee_rungs"].as_integer().unwrap() as usize;
        assert_eq!(r.len() + melee, 9);
    }
    assert_eq!(sim.app.world().resource::<GunRulesNote>().0, None);
}

#[test]
fn an_allowed_gun_of_a_game_off_can_be_dealt_and_rules_leaving_nothing_are_said() {
    let mut sim = ugg();
    let off = all_off_but(&sim, "cs_source");
    run(&mut sim, &[&off, "mp_guns_allow combat_arms:rpg-7"]);
    let mut seen = false;
    for seed in 0..60u64 {
        run(&mut sim, &[&format!("ugg_new {seed}")]);
        let r = rungs(sim.app.world());
        assert!(r.iter().all(|id| id.starts_with("cs_source:") || id == "combat_arms:rpg-7"), "{r:?}");
        seen |= r.iter().any(|id| id == "combat_arms:rpg-7");
    }
    assert!(seen, "the allowed RPG-7 comes up");
    // Every game off: every gun plays, and the note says why.
    let off = all_off_but(&sim, "");
    run(&mut sim, &["mp_guns_allow \"\"", &off, "ugg_new 3"]);
    assert!(!rungs(sim.app.world()).is_empty());
    assert!(sim.app.world().resource::<GunRulesNote>().0.is_some());
}

#[test]
fn rungs_outside_the_bounds_snap_back() {
    let mut sim = ugg();
    run(&mut sim, &["ugg_rungs 99", "ugg_new 1"]);
    assert_eq!(sim.app.world().resource::<ultimate_gun_game::AutoLadder>().rungs, 40);
}
