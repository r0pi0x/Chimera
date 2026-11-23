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

/// Submits the command line's `+` lines as the server does
/// (`ServerPlugin::build`), before the plugins' `finish` draws the launch
/// ladder.
struct CommandLine(Vec<&'static str>);

impl bevy::prelude::Plugin for CommandLine {
    fn build(&self, app: &mut bevy::prelude::App) {
        let mut console = app.world_mut().resource_mut::<Console>();
        for l in &self.0 {
            console.submit(*l);
        }
    }
}

#[test]
fn a_game_off_on_the_command_line_shapes_the_first_ladder() {
    use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim};
    let launch = |line: &'static str| {
        let mut sim = Sim::new(
            combat_arms::harness::CaSim,
            (
                GreyboxMapPlugin,
                party_roster::PartyPartsPlugin,
                CommandLine(vec![line, "ugg_rungs 12", "minigame ultimate_gun_game:auto"]),
                ultimate_gun_game::UltimateGunGamePlugin::default(),
            ),
        );
        sim.ticks(3);
        let r = rungs(sim.app.world());
        (sim, r)
    };
    let (_, base) = launch("mp_guns_ban none");
    assert!(base.iter().any(|id| id.starts_with("hl2dm:")), "HL2:DM's guns are in a default ladder: {base:?}");
    for seed_try in ["mp_guns_games_off hl2dm", "mp_guns_games_off \"hl2dm\""] {
        let (sim, r) = launch(seed_try);
        assert!(!r.iter().any(|id| id.starts_with("hl2dm:")), "{seed_try}: no HL2:DM guns: {r:?}");
        let melee = sim.app.world().resource::<chimera_sim::minigame::Minigames>().get(ultimate_gun_game::MINIGAME).unwrap().rules.params["melee_rungs"].as_integer().unwrap() as usize;
        assert_eq!(r.len() + melee, 12, "ugg_rungs from the command line too");
    }
}

/// The sandbox's ladder (`rung` reads it) leaves out a game turned off on
/// the command line, before the `+rung` there and before its match starts:
/// the launch's sandbox definition was made with every game, so it is made
/// again as the match starts (`sandbox::regenerate_on_start`).
#[test]
fn a_game_off_on_the_command_line_shapes_the_sandbox_ladder_rung_reads() {
    use bevy::prelude::*;
    use chimera_core::core::LocalPlayer;
    use chimera_sim::{minigame, minigame::ladder::Ladder, testkit};
    use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim};
    use ultimate_gun_game::sandbox::SANDBOX;
    let off: &'static str = Box::leak(all_off_but(&ugg(), "combat_arms").into_boxed_str());
    let mut sim = Sim::new(
        combat_arms::harness::CaSim,
        (
            GreyboxMapPlugin,
            party_roster::PartyPartsPlugin,
            CommandLine(vec![off, "rung 1"]),
            ultimate_gun_game::UltimateGunGamePlugin::default(),
        ),
    );
    sim.ticks(3);
    // Made at launch, before the `+` lines ran: every game's guns.
    let launch = sim.app.world().resource::<minigame::Minigames>().get(SANDBOX).expect("the sandbox is made at launch");
    let ladder: Vec<&str> = launch.rules.params["ladder"].as_array().unwrap().iter().filter_map(|v| v.as_str()).collect();
    assert!(ladder.iter().any(|id| !id.starts_with("combat_arms:")), "the launch's sandbox ladder: {ladder:?}");
    minigame::start(sim.app.world_mut(), SANDBOX).expect("the sandbox starts");
    let me = sim.spawn_character(Vec3::new(0.0, 1.0, 0.0), combat_arms::game::movement::ID);
    sim.app.world_mut().entity_mut(me).insert(LocalPlayer);
    sim.seconds(1.5);
    let ladder = sim.app.world().resource::<Ladder>().clone();
    assert!(ladder.rungs.iter().all(|id| id.starts_with("combat_arms:")), "the playing ladder: {:?}", ladder.rungs);
    let held = testkit::active_id(sim.app.world(), me);
    assert_eq!(held, Some(ladder.rungs[0]), "`+rung 1` put the player on the first rung");
}
