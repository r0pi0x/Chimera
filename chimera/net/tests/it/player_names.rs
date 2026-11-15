//! Player names on the server (`server::NAME_MAX`, `clean_name`,
//! `unique_name`): a name is at most 20 characters, its control
//! characters out and its spaces trimmed, when a player joins, renames
//! (`name`, the server's `NameRequest`) or sends it in its userinfo; an
//! empty one is "Player <id>"; a name another player has gets a number
//! within the 20.

use std::time::Duration;

use bevy::prelude::*;
use chimera_core::{console::Console, core::UserInfo, slots::Loadout};
use chimera_net::{
    harness::NetSim,
    net::{NetCharacter, NetSettings, memory::LinkConditions, server::{NAME_MAX, Player}},
};
use chimera_sim::{SimBasePlugins, movement::placeholder};
use chimera_world::greybox::GreyboxMapPlugin;

fn setup(app: &mut App) {
    app.add_plugins(GreyboxMapPlugin).insert_resource(Loadout { movement: placeholder::ID });
}

/// A client named `name`, its `name` cvar sent in its userinfo too.
fn named(name: &'static str) -> impl FnOnce(&mut World) {
    move |w| {
        w.resource_mut::<NetSettings>().name = name.into();
        w.resource_mut::<Console>().userinfo("name");
    }
}

/// The server's names: (the player's, its character's), sorted.
fn names(sim: &mut NetSim) -> Vec<String> {
    let world = sim.server.app.world_mut();
    let mut out: Vec<String> = world
        .query::<&Player>()
        .iter(world)
        .map(|p| {
            let shown = &world.get::<NetCharacter>(p.character).unwrap().name;
            assert_eq!(shown, &p.name, "the character is called as the player");
            p.name.clone()
        })
        .collect();
    out.sort();
    out
}

const W20: &str = "WWWWWWWWWWWWWWWWWWWW";

#[test]
fn names_are_cut_to_twenty_cleaned_and_kept_apart() {
    let link = LinkConditions { latency: Duration::from_millis(20), jitter: Duration::ZERO, loss: 0.0 };
    let mut sim = NetSim::new(SimBasePlugins, link, 3, 0, setup);
    assert_eq!(NAME_MAX, 20);
    sim.add_client(named("WWWWWWWWWWWWWWWWWWWWWWWWWWWWWW"));
    sim.add_client(named("  An\u{7}n\t\u{1b} "));
    sim.add_client(named("\u{7} \n"));
    sim.add_client(named("wwwwwwwwwwwwwwwwwwwwXYZ"));
    sim.until_joined(600);
    sim.ticks(10);
    let n = names(&mut sim);
    assert_eq!(n, ["Ann", "Player 3", W20, "wwwwwwwwwwwwwwwwww 2"], "{n:?}");
    assert!(n.iter().all(|n| n.chars().count() <= NAME_MAX));
    // The userinfo's name as the name is kept.
    let w = sim.server.app.world_mut();
    let infos: Vec<String> = w.query::<&UserInfo>().iter(w).filter_map(|u| u.0.get("name").cloned()).collect();
    assert!(infos.contains(&W20.to_string()), "{infos:?}");
    assert!(infos.iter().all(|n| n.chars().count() <= NAME_MAX && !n.chars().any(char::is_control)), "{infos:?}");

    // A rename: cut, cleaned, kept apart from the others (" 2" is taken,
    // case aside).
    sim.clients[1].app.world_mut().resource_mut::<NetSettings>().name = format!("{W20}{W20}");
    sim.ticks(30);
    let n = names(&mut sim);
    assert_eq!(n, ["Player 3", "WWWWWWWWWWWWWWWWWW 3", W20, "wwwwwwwwwwwwwwwwww 2"], "{n:?}");
    // And to a free name, cleaned.
    sim.clients[1].app.world_mut().resource_mut::<NetSettings>().name = "Bob\u{0}".into();
    sim.ticks(30);
    assert!(names(&mut sim).contains(&"Bob".to_string()), "{:?}", names(&mut sim));
}
