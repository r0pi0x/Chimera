//! What the games that are not in the match cost a client's frame
//! (docs/plans/active/game-logic-distribution.md, section 7): the
//! party's client adds every game's front end (`party_roster::client::
//! CLIENTS`: each guest's HUD on the shell, `GameClient::hud`, and the rest
//! of it, `GameClient::app`), and each guest's front-end systems run only
//! while its game is in play (`party_kit::client::gate_front_ends`, on
//! `chimera_core::games_in_play`). `idle_games`' match (Combat Arms' guns
//! only, eight bots on the greybox) with a local player and the guests'
//! front ends as the client adds them, headless (the shell's own plugins
//! left out: no renderer), counted per system by work counts (frames of
//! one tick: the `Sim` runs every schedule each update) and summed by
//! crate: the absent games' systems, simulation and front end, must cost
//! under 1% of the frame; the same frame with the front ends ungated (as
//! before) is printed beside it. A game entering play mid-match runs its
//! front end's systems in that same frame.
//! `cargo test -p ultimate_gun_game --features dev --test it
//! heavy::idle_front_ends -- --nocapture`.

use bevy::{
    app::{PluginGroup, PluginGroupBuilder},
    prelude::*,
};
use chimera_client::client::{
    game_text::GameTextPlugin, hud::HudPlugin, view::ViewPlugin, weapon_menu::WeaponMenuPlugin,
};
use chimera_core::{
    core::{LocalPlayer, SpawnPoint},
    games_in_play::{self, GamesInPlay},
};
use chimera_sim::minigame::{self, ladder::Ladder};
use chimera_world::harness::Sim;
use combat_arms::harness::CaSim;
use party_roster::client::{CLIENT, CLIENTS};
use ultimate_gun_game::{MINIGAME, RUNGS, UltimateGunGamePlugin, generate};

/// The shell's plugins the games' HUDs place theirs after (`add_after`),
/// disabled once they have: only the games' own are added.
struct ShellAnchors;

impl PluginGroup for ShellAnchors {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>()
            .add(HudPlugin)
            .add(ViewPlugin)
            .add(GameTextPlugin)
            .add(WeaponMenuPlugin)
    }
}

/// The guests' front ends as the party's client adds them (`party_kit::
/// client::with_huds`, `front_end`), gated (`gate_front_ends`) or not.
fn front_ends(app: &mut App, gated: bool) {
    let since = games_in_play::systems_now(app);
    // What the HUDs read of the shell and write to (no renderer here).
    app.init_resource::<chimera_client::client::fonts::UiFonts>()
        .init_resource::<chimera_client::client::console::HeldActions>()
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<ButtonInput<MouseButton>>()
        .init_asset::<chimera_client::client::hud_text::HudGlyphMaterial>()
        .init_asset::<Image>()
        .init_asset::<bevy::shader::Shader>();
    let base = CLIENT.party.base.info.id;
    let mut group = ShellAnchors.build();
    for client in CLIENTS.iter().filter(|c| c.game != base) {
        if let Some(hud) = client.hud {
            group = hud(group);
        }
    }
    let group = group.disable::<HudPlugin>().disable::<ViewPlugin>().disable::<GameTextPlugin>();
    app.add_plugins(group);
    for client in CLIENTS.iter().filter(|c| c.game != base) {
        if let Some(add) = client.app {
            add(app);
        }
    }
    if gated {
        let n = party_roster::client::gate_front_ends(app, &since, &CLIENT);
        eprintln!("front ends: {n} systems gated");
    }
}

/// `idle_games`' match with a local player and the guests' front ends.
fn client_match(gated: bool) -> Sim {
    let mut sim = Sim::with(CaSim, |app| {
        app.add_plugins((
            chimera_world::greybox::GreyboxMapPlugin,
            party_roster::PartyPartsPlugin,
            UltimateGunGamePlugin::default(),
        ));
        front_ends(app, gated);
    });
    let w = sim.app.world_mut();
    w.insert_resource(chimera_net::net::manifest::LocalMounts(chimera_net::net::manifest::MountReport {
        games: vec![chimera_net::net::manifest::GameMount {
            game: "combat_arms".to_string(),
            installed: true,
            ..Default::default()
        }],
    }));
    sim.repeatable();
    for x in [-8.0, 8.0] {
        for z in [-25.0, -15.0, -5.0, 5.0] {
            sim.app.world_mut().spawn((SpawnPoint::default(), Transform::from_translation(Vec3::new(x, 1.0, z))));
        }
    }
    {
        let mut s = sim.app.world_mut().resource_mut::<combat_arms::game::bots::BotSettings>();
        s.quota = 7;
        s.quota_set = true;
    }
    let w = sim.app.world_mut();
    generate(w, 7, RUNGS).expect("a ladder");
    chimera_core::gameplay::new_game(w);
    minigame::start(w, MINIGAME).expect("the match starts");
    let me = sim.spawn_character(Vec3::new(0.0, 1.0, 0.0), combat_arms::game::movement::ID);
    sim.app.world_mut().entity_mut(me).insert(LocalPlayer);
    sim.seconds(3.0);
    sim
}

/// The gated games, and the crates of their systems: their own and their
/// front ends'.
fn absent_crates(world: &World) -> Vec<String> {
    let games: Vec<&str> = world.resource::<GamesInPlay>().games().filter(|(_, on)| !on).map(|(g, _)| g).collect();
    CLIENTS
        .iter()
        .filter(|c| games.contains(&c.game))
        .flat_map(|c| [c.game.to_string(), c.ui_crate.to_string()])
        .collect()
}

#[test]
fn absent_games_front_ends_cost_under_a_percent_of_a_frame() {
    const FRAMES: u64 = 300;
    let mut rows = Vec::new();
    for gated in [true, false] {
        let mut sim = client_match(gated);
        let why = sim.app.world().resource::<GamesInPlay>().why.clone();
        let absent = absent_crates(sim.app.world());
        assert_eq!(absent.len(), 2 * (CLIENTS.len() - 1), "every guest is out of play: {why:?}");
        let Some(work) = sim.work(FRAMES) else {
            eprintln!("skipping: no hardware counters");
            return;
        };
        let frame = work.total() / FRAMES;
        let cost = |crates: &[String]| crates.iter().map(|c| work.crates.get(c).map_or(0, |v| v.0)).sum::<u64>() / FRAMES;
        let ui: Vec<String> = absent.iter().filter(|c| c.ends_with("_ui")).cloned().collect();
        let (all, fronts) = (cost(&absent), cost(&ui));
        eprintln!(
            "{}: {frame} instructions a frame; absent games' systems {all} ({:.2}%), their front-end crates' {fronts}",
            if gated { "front ends gated" } else { "front ends ungated (as before)" },
            100.0 * all as f64 / frame as f64
        );
        for c in &absent {
            if let Some((ins, runs)) = work.crates.get(c) {
                eprintln!("  {c:<16} {:>10} instructions, {:>5} runs a frame", ins / FRAMES, runs / FRAMES);
            }
            let mut top: Vec<_> = work.crate_systems.iter().filter(|(k, _)| k.split(' ').next() == Some(c)).collect();
            top.sort_by(|a, b| b.1.0.cmp(&a.1.0));
            let top: Vec<_> = top.iter().take(5).map(|(k, v)| (k.as_str(), v.0 / FRAMES, v.1 / FRAMES)).collect();
            if !top.is_empty() {
                eprintln!("    costliest (instructions, runs a frame): {top:?}");
            }
        }
        if gated {
            assert_eq!(fronts, 0, "no absent game's front-end system runs (party_kit::client::gate_front_ends)");
        }
        rows.push((frame, all));
    }
    let ((gated, absent), (ungated, _)) = (rows[0], rows[1]);
    eprintln!(
        "frame: {gated} gated, {ungated} with the front ends ungated: {:.1}% saved",
        100.0 * (ungated as f64 - gated as f64) / ungated as f64
    );
    assert!(
        absent * 100 < gated,
        "the games not in play cost {absent} instructions of a {gated}-instruction frame (bound: 1%): a guest's \
         front-end system runs while its game is out of play (party_kit::client::gate_front_ends)"
    );
}

#[test]
fn a_game_entering_play_mid_match_runs_its_front_end_that_frame() {
    let mut sim = client_match(true);
    let world = sim.app.world_mut();
    let me = world.query_filtered::<Entity, With<LocalPlayer>>().single(world).expect("the local player");
    assert!(!world.resource::<GamesInPlay>().is_in_play("halo_ce"));
    // The local player's rung a gun of a game the match had none of
    // (Halo's: its front end's systems run every frame, its HUD up or
    // not; HL2:DM's wait for its HUD).
    let mut ladder = world.resource::<Ladder>().clone();
    ladder.rungs[0] = "halo_ce:assault_rifle";
    minigame::ladder::set_rung(world, me, 0, &ladder);
    assert!(world.resource::<GamesInPlay>().is_in_play("halo_ce"), "Halo is in play once its gun is in hand");
    let Some(work) = sim.work(1) else {
        eprintln!("skipping the systems' runs: no hardware counters");
        return;
    };
    let runs = work.crates.get("halo_ce_ui").map_or(0, |v| v.1);
    assert!(runs > 0, "Halo's front end ran in the frame it entered play");
    assert!(work.crates.get("tf2_ui").is_none_or(|v| v.1 == 0), "TF2's stays off: {:?}", work.crates.get("tf2_ui"));
}
