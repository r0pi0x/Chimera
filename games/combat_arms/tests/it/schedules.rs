//! Combat Arms' simulation schedules as its dedicated server and its
//! client compose them (docs/plans/active/architecture-review.md, E1
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
//! (apps/combat_arms_server/src/lib.rs's `ServerPlugin`,
//! apps/combat_arms/src/main.rs on the greybox), since the order systems
//! are added in decides how Bevy sorts the unordered ones; headless and
//! without what lies above the game. The client's map is the engine's
//! `MapSimPlugin` where the game adds the presentation's `MapPlugin`
//! (this crate's tests don't link the presentation), and the client shell
//! and Combat Arms' front end (`ui`) aren't in this binary, so their
//! systems in the fixed schedules are not compared.

use bevy::prelude::*;
use chimera_net::net::NetPlugin;
use chimera_world::{
    greybox::GreyboxMapPlugin,
    harness::{ScheduleRatchet, add_headless, check_schedule_ratchet},
    map::MapSimPlugin,
};
use combat_arms::game::CombatArmsPlugins;

/// Per schedule: ambiguous pairs (conflicting access, no order) on the
/// server and on the client, and how many of them the two run in
/// opposite orders. Each may fall (lower it here), never rise
/// (`check_schedule_ratchet` says which pairs to order). The prediction
/// schedules' opposite pairs stay at 0.
const RATCHET: &[ScheduleRatchet] = &[
    ScheduleRatchet {
        schedule: "FixedUpdate",
        server: 550,
        client: 550,
        opposite: 91,
    },
    ScheduleRatchet {
        schedule: "FixedPostUpdate",
        server: 13,
        client: 13,
        opposite: 4,
    },
    ScheduleRatchet {
        schedule: "FixedLast",
        server: 97,
        client: 97,
        opposite: 32,
    },
    ScheduleRatchet {
        schedule: "Predict::Select",
        server: 5,
        client: 5,
        opposite: 0,
    },
    ScheduleRatchet {
        schedule: "Predict::PlaceMovers",
        server: 0,
        client: 0,
        opposite: 0,
    },
    ScheduleRatchet {
        schedule: "Predict::CarryRiders",
        server: 0,
        client: 0,
        opposite: 0,
    },
    ScheduleRatchet {
        schedule: "Predict::Movement",
        server: 4,
        client: 4,
        opposite: 0,
    },
    ScheduleRatchet {
        schedule: "Predict::Weapons",
        server: 12,
        client: 12,
        opposite: 0,
    },
];

/// The dedicated server's composition on the greybox (`ServerPlugin`).
fn server() -> App {
    let mut app = App::new();
    add_headless(&mut app);
    app.add_plugins(combat_arms::SimPlugins)
        .add_plugins((GreyboxMapPlugin, MapSimPlugin::empty()))
        .add_plugins((NetPlugin, CombatArmsPlugins));
    app.finish();
    app.cleanup();
    app
}

/// The game's (a client or listen server) on the greybox: the map first,
/// then the engine, the simulation (`combat_arms_app::SimPlugin`) and the
/// network.
fn client() -> App {
    let mut app = App::new();
    app.add_plugins((GreyboxMapPlugin, MapSimPlugin::empty()));
    add_headless(&mut app);
    app.add_plugins((combat_arms::SimPlugins, CombatArmsPlugins))
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

/// Each mode's own per-tick systems (plan ecs-cleanup step 4: gated by
/// `mode::mode_is`, in place of the runtime's update and touches
/// dispatchers) run where those ran, on the server and the client: the
/// updates after `update_team_counts` and before the phases' step
/// (`flow::mission_update`), a mode's own steps in order and a clan mode's
/// after its base mode's; the touches after the tick's deaths
/// (`scoring::handle_deaths`) and before `publish`.
#[test]
fn mode_systems_run_where_the_dispatchers_ran() {
    use chimera_world::harness::schedule_systems;
    const UPDATES: &[&str] = &[
        "search_destroy::hand_out",
        "search_destroy::bombs",
        "ctf::update",
        "clan::ctf_half",
        "clan::leaver_ends_round",
        "bombing_run::carry",
        "bombing_run::bombs",
        "seize::update",
        "vip::update",
        "arms_race::update",
        "hunted::update",
        "spy_hunt::update",
        "quarantine::update",
        "hired_guns::update",
        "escort::update",
    ];
    const TOUCHES: &[&str] = &["hired_guns::touches", "escort::touches"];
    for (name, mut app) in [("server", server()), ("client", client())] {
        let order = schedule_systems(app.world_mut(), FixedUpdate);
        let find = |what: &str, from: usize| {
            order
                .iter()
                .skip(from)
                .position(|s| s == what)
                .map(|i| i + from)
                .unwrap_or_else(|| panic!("{name}: {what} not after {from} in {order:#?}"))
        };
        let rules = |what: &str| format!("combat_arms::game::rules::{what}");
        let counts = find(&rules("update_team_counts"), 0);
        let step = find("chimera_sim::mode::flow::mission_update", 0);
        let mut last = counts;
        for u in UPDATES {
            let at = find(&rules(u), 0);
            assert!(last < at && at < step, "{name}: {u} at {at}, not between {last} and the step at {step}");
            last = at;
        }
        assert!(!order.iter().any(|s| s.starts_with("chimera_sim::mode::flow::update")));
        let deaths = find(&rules("scoring::handle_deaths"), 0);
        let mut last = deaths;
        for t in TOUCHES {
            let at = find(&rules(t), 0);
            assert!(last < at, "{name}: {t} at {at}, before {last}");
            last = at;
        }
        let publish = find(&rules("publish"), last);
        assert!(last < publish);
    }
}

/// Plan ecs-cleanup step 5: the server's death systems are normal
/// systems reading `Died`, their world work (the marking and counting,
/// the modes' drops; the perks' boxes and Parting Gift's grenade) queued
/// and applied at a sync point before what reads it in the same tick (the
/// modes' touches, `perks::supplies`), as the exclusive versions' writes
/// were.
#[test]
fn the_deaths_world_work_applies_before_its_readers() {
    use chimera_world::harness::schedule_systems;
    let mut app = server();
    let order = schedule_systems(app.world_mut(), FixedUpdate);
    // The first `what` from `from` on (`publish` runs twice a tick).
    let at = |what: &str, from: usize| {
        let full = format!("combat_arms::game::{what}");
        let i = order[from..].iter().position(|s| *s == full).unwrap_or_else(|| panic!("{full} not after {from} in {order:#?}"));
        i + from
    };
    for (writer, reader) in [
        ("rules::scoring::handle_deaths", "rules::hired_guns::touches"),
        ("rules::scoring::handle_deaths", "rules::escort::touches"),
        ("rules::scoring::handle_deaths", "rules::publish"),
        ("perks::perk_deaths", "perks::supplies"),
    ] {
        let w = at(writer, 0);
        let r = at(reader, w);
        assert!(order[w + 1..r].iter().any(|s| s.ends_with("apply_deferred")), "{writer} at {w}, no sync point before {reader} at {r}");
    }
}
