//! A network client's own movement stays exact and smooth for every
//! game's character on another game's map (the party's server on the
//! greybox, `NetSim`, 30 ms each way, the map taken as CS:S's so every
//! other game's body is fitted to its envelope: `chimera_core::envelope`).
//! Per rung (CS:S, HL2:DM, TF2, Combat Arms, Halo): walking, turning,
//! jumping and crouching, the client predicts every state the server
//! sends (`NetGraph::errors` and `resyncs` 0); then drawn at 144 frames a
//! second (between ticks, `map::interp`), the body circling at a steady
//! speed (a circle a second) moves every frame (no frame held still) by
//! steps that change frame to frame by no more than turning at the ticks
//! gives (a quarter of the step and 1 mm; a correction, or a tick drawn
//! late or twice, changes a step by all of it). The signals a "feels
//! laggy" report is checked against (2026-11-21: 0 errors and 0 restarts
//! for every rung; walking straight, steps changing by at most 1.5 mm,
//! the same as before that day's third-person and character-fit work).

use std::time::Duration;

use bevy::prelude::*;
use chimera_core::core::MapGame;
use chimera_net::{
    harness::NetSim,
    net::{memory::LinkConditions, predict::NetGraph},
};
use chimera_sim::minigame::{self, MinigameAppExt, ladder::Ladder};
use chimera_world::{greybox::GreyboxMapPlugin, map::MapSimPlugin};
use combat_arms::{game::TICK_INTERVAL, harness::CaSim};
use party_roster::PartyPartsPlugin;

const LADDER: [&str; 5] = [
    "cs_source:weapon_glock",
    "hl2dm:weapon_smg1",
    "tf2:tf_weapon_scattergun",
    "combat_arms:m4a1",
    halo_ce::game::weapons::ASSAULT_RIFLE,
];

fn party() -> NetSim {
    let text = crate::hl2dm_parts::ladder_text(&LADDER, false).replace(
        "weapons = [\"cs_source\", \"hl2dm\", \"combat_arms\"]",
        "weapons = [\"cs_source\", \"hl2dm\", \"halo_ce\", \"tf2\", \"combat_arms\"]",
    );
    let link = LinkConditions {
        latency: Duration::from_millis(30),
        jitter: Duration::ZERO,
        loss: 0.0,
    };
    let mut sim = NetSim::new(CaSim, link, 5, 1, |app| {
        app.add_plugins((GreyboxMapPlugin, MapSimPlugin::empty(), PartyPartsPlugin, chimera_world::map::interp::InterpPlugin))
            .insert_resource(MapGame("cs_source".into()));
    });
    crate::gun_game::plain_rungs(sim.server.app.world_mut());
    crate::gun_game::no_finale(sim.server.app.world_mut());
    sim.server.app.add_minigames(&[("net_smoothness.toml", &text)]).unwrap();
    minigame::start(sim.server.app.world_mut(), "test:gun_game_hl2dm").expect("the gun game starts");
    sim.set_frame(TICK_INTERVAL);
    sim.until_joined(900);
    sim.until_looks_taken(600);
    sim.ticks(120);
    sim
}

/// The server's character of client 0 at the greybox's middle.
fn to_the_middle(sim: &mut NetSim) {
    let e = sim.character_of(0).unwrap();
    let p = Vec3::new(0.0, 1.0, 0.0);
    let w = sim.server.app.world_mut();
    w.get_mut::<Transform>(e).unwrap().translation = p;
    w.get_mut::<avian3d::prelude::Position>(e).unwrap().0 = p;
}

#[test]
fn a_client_s_own_movement_is_exact_and_smooth_for_every_game_s_body() {
    let mut sim = party();
    let ladder = sim.server.app.world().resource::<Ladder>().clone();
    let mut failures = Vec::new();
    for (rung, gun) in LADDER.iter().enumerate() {
        let e = sim.character_of(0).unwrap();
        minigame::ladder::set_rung(sim.server.app.world_mut(), e, rung, &ladder);
        sim.set_frame(TICK_INTERVAL);
        sim.ticks(60);
        to_the_middle(&mut sim);
        sim.ticks(120);
        let me = sim.local_player(0).unwrap();
        *sim.clients[0].app.world_mut().resource_mut::<NetGraph>() = NetGraph::default();
        // Walking in a circle (2 m or so), jumping and crouching now and then.
        for t in 0..400u32 {
            let mut i = sim.clients[0].intent(me);
            i.yaw = (t as f32 * 0.03).rem_euclid(std::f32::consts::TAU);
            i.pitch = 0.0;
            i.move_axis = Vec2::Y;
            i.jump = t % 90 < 5;
            i.crouch = t % 240 > 200;
            drop(i);
            sim.step();
        }
        // Between ticks: 144 frames a second, circling steadily (a circle
        // a second, 2 m across or less, clear of the greybox's walls).
        let walked = sim.clients[0].app.world().resource::<NetGraph>().clone();
        sim.clients[0].intent(me).move_axis = Vec2::ZERO;
        sim.ticks(30);
        to_the_middle(&mut sim);
        sim.ticks(60);
        *sim.clients[0].app.world_mut().resource_mut::<NetGraph>() = NetGraph::default();
        sim.set_frame(1.0 / 144.0);
        let mut at = Vec::new();
        for f in 0..600u32 {
            let mut i = sim.clients[0].intent(me);
            i.yaw = (f as f32 * std::f32::consts::TAU / 144.0).rem_euclid(std::f32::consts::TAU);
            i.move_axis = Vec2::Y;
            i.jump = false;
            i.crouch = false;
            drop(i);
            sim.step();
            at.push(sim.clients[0].app.world().get::<Transform>(me).unwrap().translation.xz());
        }
        // From a second in (up to speed).
        let steps: Vec<Vec2> = at.windows(2).skip(144).map(|w| w[1] - w[0]).collect();
        let held = steps.iter().filter(|s| s.length() < 1e-6).count();
        // Each step's change against what turning at the ticks gives: a
        // quarter of the step and a millimetre at most (a tick drawn late
        // or twice, or a correction, changes it by all of it).
        let worst = steps.windows(2).map(|w| (w[1] - w[0]).length() / (0.25 * w[0].length() + 0.001)).fold(0.0f32, f32::max);
        let drawn = sim.clients[0].app.world().resource::<NetGraph>().clone();
        let mut g = drawn.clone();
        g.checked += walked.checked;
        g.errors += walked.errors;
        g.resyncs += walked.resyncs;
        g.worst_error = g.worst_error.max(walked.worst_error);
        if drawn.errors == 0 {
            g.last_differing = walked.last_differing.clone();
        }
        println!(
            "{gun}: {} states checked, {} errors (worst {:.4} m), {} resyncs; drawn: {held} frames held, step change at most {:.2} of its allowance",
            g.checked,
            g.errors,
            g.worst_error,
            g.resyncs,
            worst
        );
        if g.checked == 0 || g.errors != 0 || g.resyncs != 0 || held != 0 || worst > 1.0 {
            failures.push(format!(
                "{gun}: {} checked, {} errors ({:?}), {} resyncs, {held} frames held, step change {worst} of its allowance",
                g.checked, g.errors, g.last_differing, g.resyncs
            ));
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}
