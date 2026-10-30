//! CS:S's simulation schedules as its dedicated server and its client
//! compose them (docs/plans/active/architecture-review.md, E1
//! steps 2 and 3) as one ratchet (`chimera_world::harness::
//! check_schedule_ratchet`): per schedule, the ambiguous pairs
//! (conflicting systems with no order between them) of each, and how
//! many of those the server and the client run in opposite orders (where
//! a client computes otherwise than its server: a misprediction); each
//! count may only fall. Prediction's schedules have no such pair; the
//! fixed tick's still have some (the stored numbers), so a strict
//! "same order" test would first need them all ordered.
//!
//! Each composition adds the plugins in its binary's order
//! (apps/cs_source_server/src/main.rs, apps/cs_source/src/main.rs on the
//! greybox), since the order systems are added in decides how Bevy sorts
//! the unordered ones; headless (no window, rendering or audio) and
//! without what lies above the game: the client shell and CS:S's front
//! end (`ui`) aren't in this binary, so their systems in the fixed
//! schedules (the shell's input and interpolation, the objectives HUD)
//! are not compared.

use bevy::prelude::*;
use chimera_net::net::NetPlugin;
use chimera_world::{
    greybox::GreyboxMapPlugin,
    harness::{ScheduleRatchet, add_headless, check_schedule_ratchet},
    map::MapSimPlugin,
};
use cs_source::game::{CsSourcePlugins, player_anim::PlayerAnimPlugin, view_anim::ViewAnimPlugin};

/// Per schedule: ambiguous pairs (conflicting access, no order) on the
/// server and on the client, and how many of them the two run in
/// opposite orders. Each may fall (lower it here), never rise
/// (`check_schedule_ratchet` says which pairs to order). The prediction
/// schedules' opposite pairs stay at 0.
const RATCHET: &[ScheduleRatchet] = &[
    ScheduleRatchet {
        schedule: "FixedUpdate",
        server: 235,
        client: 235,
        opposite: 40,
    },
    ScheduleRatchet {
        schedule: "FixedPostUpdate",
        server: 187,
        client: 189,
        opposite: 30,
    },
    ScheduleRatchet {
        schedule: "FixedLast",
        server: 77,
        client: 77,
        opposite: 17,
    },
    ScheduleRatchet {
        schedule: "Predict::Select",
        server: 0,
        client: 0,
        opposite: 0,
    },
    ScheduleRatchet {
        schedule: "Predict::Movement",
        server: 3,
        client: 3,
        opposite: 0,
    },
    ScheduleRatchet {
        schedule: "Predict::Weapons",
        server: 9,
        client: 9,
        opposite: 0,
    },
];

/// The dedicated server's composition on the greybox.
fn server() -> App {
    let mut app = App::new();
    add_headless(&mut app);
    app.add_plugins(cs_source::SimPlugins)
        .add_plugins((GreyboxMapPlugin, MapSimPlugin::empty()))
        .add_plugins((
            NetPlugin,
            CsSourcePlugins
                .build()
                .disable::<PlayerAnimPlugin>()
                .disable::<ViewAnimPlugin>(),
        ));
    app.finish();
    app.cleanup();
    app
}

/// The game's (a client or listen server) on the greybox: the map with
/// what draws and plays it first, then the engine, the simulation and
/// the network.
fn client() -> App {
    let mut app = App::new();
    app.add_plugins((GreyboxMapPlugin, chimera_present::MapPlugin::empty()));
    add_headless(&mut app);
    app.add_plugins((cs_source::SimPlugins, CsSourcePlugins))
        .add_plugins(NetPlugin);
    app.finish();
    app.cleanup();
    app
}

#[test]
fn schedule_ratchet() {
    if let Err(report) = check_schedule_ratchet(&mut server(), &mut client(), RATCHET) {
        panic!("{report}");
    }
}

/// Plan ecs-cleanup step 5: the weapon layer's removals, new characters'
/// kits and the map's equips are normal systems in their old order with
/// no sync point between them (their world work applies together, in
/// that order, at the one before `pick_up`), so the rest of the tick
/// keeps the order it had when the first two were exclusive.
#[test]
fn the_weapon_layers_givers_keep_their_place() {
    use chimera_world::harness::schedule_systems;
    for (name, mut app) in [("server", server()), ("client", client())] {
        let order = schedule_systems(app.world_mut(), FixedUpdate);
        let at = |what: &str| {
            let full = format!("chimera_sim::weapon::{what}");
            order.iter().position(|s| *s == full).unwrap_or_else(|| panic!("{name}: {full} not in {order:#?}"))
        };
        let syncs = |a: usize, b: usize| order[a + 1..b].iter().filter(|s| s.ends_with("apply_deferred")).count();
        let chain = ["equip::map_equipment", "equip::remove_killed_weapons", "give_starting_weapons", "equip::apply_equips"].map(at);
        assert!(chain.windows(2).all(|w| w[0] < w[1] && syncs(w[0], w[1]) == 0), "{name}: {chain:?}");
        let pick_up = at("drop::pick_up");
        assert!(chain[3] < pick_up && syncs(chain[3], pick_up) == 1, "{name}: one sync point before pick_up at {pick_up}");
    }
}
