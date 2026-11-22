//! The app's startup path, headless: the command line as the binary reads
//! it (`Args::parse_from`, then `with_defaults`), its console lines
//! submitted before the first frame as the client shell does
//! (`chimera_client::client::console`'s `autoexec`), on the party's
//! simulation. The minigame must be playing once the first frames ran,
//! whichever the launch order: the lines before the player exists, the
//! player there first, or both in one frame.

use bevy::prelude::*;
use chimera_client::client::Args;
use chimera_core::{
    console::Console,
    core::{Dead, LocalPlayer},
    gameplay::Gameplay,
};
use chimera_sim::{
    minigame::{ActiveMinigame, MINIGAME, hud::MinigameHud, ladder::Rung},
    mode::{Match, Phase},
    testkit,
};
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim};
use combat_arms::{game::movement, harness::CaSim};
use party_roster::{DEFAULT_MINIGAME, PartyPartsPlugin, client::with_defaults};

/// The console lines the app runs for `args` (its words as typed).
fn lines(args: &str) -> Vec<String> {
    let args = Args::parse_from(args.split_whitespace().map(str::to_string)).expect("the command line reads");
    // A tool run (`--frames`, `--screenshot`) also gets loopback network
    // lines (`ip`, `hostport 0`), not what these tests are about.
    with_defaults(args).console.into_iter().filter(|l| !l.starts_with("ip ") && !l.starts_with("hostport ")).collect()
}

fn party() -> Sim {
    Sim::new(CaSim, (GreyboxMapPlugin, PartyPartsPlugin))
}

fn submit(sim: &mut Sim, lines: &[String]) {
    let mut console = sim.app.world_mut().resource_mut::<Console>();
    for l in lines {
        console.submit(l.clone());
    }
}

/// What the player must see once the minigame plays: its rules in the
/// gameplay, its HUD data, the first rung's gun in hand.
fn assert_playing(sim: &Sim, player: Entity) {
    let w = sim.app.world();
    assert_eq!(w.resource::<Gameplay>().rules, MINIGAME, "the minigame's rules");
    assert_eq!(w.resource::<ActiveMinigame>().def.id, DEFAULT_MINIGAME);
    let hud = w.resource::<MinigameHud>();
    assert!(hud.playing() && !hud.lines.is_empty(), "the HUD's data: {hud:?}");
    assert_eq!(w.get::<Rung>(player).map(|r| r.0), Some(0));
    assert!(w.get::<Dead>(player).is_none());
    assert_ne!(w.resource::<Match>().phase, Phase::Waiting, "the match runs");
    assert_eq!(testkit::active_id(w, player), Some(cs_source::game::weapons::GLOCK), "the first rung's gun");
}

fn local_soldier(sim: &mut Sim) -> Entity {
    let e = sim.spawn_character(Vec3::new(0.0, 1.0, 0.0), movement::ID);
    sim.app.world_mut().entity_mut(e).insert(LocalPlayer);
    e
}

#[test]
fn the_command_line_names_the_minigame_and_the_console_runs_it() {
    let l = lines("+minigame originals:gun_game_dust2 --frames 90");
    assert_eq!(l, ["MaxPlayer 3", "minigame originals:gun_game_dust2"]);
    // Without `+minigame`: the default, with its bots.
    assert_eq!(lines("--frames 90"), ["MaxPlayer 3", "minigame originals:gun_game_dust2"]);
    // The user's own bot count and extra commands stay, in order.
    let l = lines("+maxplayer 0 +god --screenshot x.png +minigame originals:gun_game_dust2");
    assert_eq!(l, ["maxplayer 0", "god", "minigame originals:gun_game_dust2"]);
}

/// The lines first (no player yet, as at startup), the player after.
#[test]
fn the_command_line_starts_the_gun_game_before_the_player_exists() {
    for args in ["+minigame originals:gun_game_dust2 --frames 90", "--frames 90"] {
        let mut sim = party();
        submit(&mut sim, &lines(args));
        sim.ticks(2);
        assert!(sim.app.world().get_resource::<ActiveMinigame>().is_some(), "{args}: started");
        let p = local_soldier(&mut sim);
        sim.seconds(1.5);
        assert_playing(&sim, p);
    }
}

/// The player there first (a map that spawned it), the lines after.
#[test]
fn the_command_line_starts_the_gun_game_with_the_player_in() {
    let mut sim = party();
    let p = local_soldier(&mut sim);
    sim.seconds(0.5);
    submit(&mut sim, &lines("+minigame originals:gun_game_dust2"));
    sim.seconds(1.5);
    assert_playing(&sim, p);
}

/// The lines in the same frame as the player's spawn.
#[test]
fn the_command_line_starts_the_gun_game_in_the_players_first_frame() {
    let mut sim = party();
    submit(&mut sim, &lines("+minigame originals:gun_game_dust2"));
    let p = local_soldier(&mut sim);
    sim.seconds(1.5);
    assert_playing(&sim, p);
}
