//! Console names on the wire (docs/plans/active/console-namespaces.md):
//! replicated cvars travel by full name, so two games' settings with one
//! short name each reach their own cvar on the client; and a client's
//! commands for its server (its cheats, our RCON) go with the command's
//! full name, so the server runs what the client meant whatever the
//! server's own scope.

use std::time::Duration;

use bevy::prelude::*;
use bevy_replicon::prelude::FromClient;
use chimera_core::{
    console::{Console, resource_cvar},
    slots::Loadout,
};
use chimera_net::{
    harness::NetSim,
    net::{cheats::ClientCommand, memory::LinkConditions},
};
use chimera_sim::{SimBasePlugins, movement::placeholder};
use chimera_world::greybox::GreyboxMapPlugin;

/// Two games' replicated settings with one short name.
#[derive(Resource, Default)]
struct Rules {
    cs: u32,
    ca: u32,
    /// An engine name (`decalfrequency`), registered bare: `chimera:`.
    engine: u32,
}

/// What the server heard from clients (`ClientCommand` lines).
#[derive(Resource, Default)]
struct Heard(Vec<String>);

fn hear(mut from: MessageReader<FromClient<ClientCommand>>, mut heard: ResMut<Heard>) {
    heard.0.extend(from.read().map(|m| m.message.line.clone()));
}

/// Whether a game's look-alike `setpos` ran (a command a server must not
/// pick for the client's `chimera:setpos`).
#[derive(Resource, Default)]
struct OtherSetpos(u32);

fn setup(app: &mut App) {
    app.add_plugins(GreyboxMapPlugin)
        .insert_resource(Loadout {
            movement: placeholder::ID,
        })
        .init_resource::<Rules>()
        .init_resource::<Heard>()
        .init_resource::<OtherSetpos>()
        .add_systems(PreUpdate, hear);
    resource_cvar::<Rules, u32>(app, "cs_source:sv_rule", "CS:S's rule", |r| &mut r.cs);
    resource_cvar::<Rules, u32>(app, "decalfrequency", "Source's, the engine's", |r| &mut r.engine);
    resource_cvar::<Rules, u32>(app, "combat_arms:sv_rule", "Combat Arms' rule", |r| &mut r.ca);
    chimera_core::console::ConsoleAppExt::console_command(
        app,
        "combat_arms:setpos",
        "Another game's setpos.",
        |w, _| {
            w.resource_mut::<OtherSetpos>().0 += 1;
            Ok(None)
        },
    );
    let mut c = app.world_mut().resource_mut::<Console>();
    c.add_namespace(&["cs_source", "css"]);
    c.add_namespace(&["combat_arms", "ca"]);
}

fn submit(sim: &mut NetSim, app: Option<usize>, line: &str) {
    let app = match app {
        Some(i) => &mut sim.clients[i].app,
        None => &mut sim.server.app,
    };
    app.world_mut().resource_mut::<Console>().submit(line);
}

fn joined() -> NetSim {
    let link = LinkConditions {
        latency: Duration::from_millis(20),
        jitter: Duration::ZERO,
        loss: 0.0,
    };
    let mut sim = NetSim::new(SimBasePlugins, link, 5, 1, setup);
    sim.until_joined(600);
    sim.ticks(30);
    sim
}

#[test]
fn replicated_cvars_reach_the_setting_of_their_full_name() {
    let mut sim = joined();
    // The server's scope is Combat Arms: a short name there is its.
    submit(&mut sim, None, "scope ca; sv_rule 7; cs_source:sv_rule 3");
    sim.ticks(30);
    assert_eq!(sim.server.app.world().resource::<Rules>().ca, 7);
    // The client, scoped to CS:S, gets each value on its own cvar: the
    // names went in full (a short `sv_rule` would be its CS:S one here).
    submit(&mut sim, Some(0), "scope css");
    let both = sim.until(300, |s| {
        let r = s.clients[0].app.world().resource::<Rules>();
        r.ca == 7 && r.cs == 3
    });
    let r = sim.clients[0].app.world().resource::<Rules>();
    assert!(both, "client: cs {} ca {}", r.cs, r.ca);
    // A change follows by full name too.
    submit(&mut sim, None, "combat_arms:sv_rule 9");
    assert!(sim.until(300, |s| s.clients[0].app.world().resource::<Rules>().ca == 9));
    assert_eq!(sim.clients[0].app.world().resource::<Rules>().cs, 3);
    // The client can't set the server's (refused by name, either name).
    submit(&mut sim, Some(0), "sv_rule 1; ca:sv_rule 1");
    sim.ticks(10);
    let r = sim.clients[0].app.world().resource::<Rules>();
    assert_eq!((r.cs, r.ca), (3, 9));
}

#[test]
fn a_clients_commands_go_to_its_server_by_full_name() {
    let mut sim = joined();
    // The server allows cheats, and its scope is Combat Arms', whose
    // `setpos` is another command.
    submit(&mut sim, None, "sv_cheats 1; scope combat_arms");
    sim.ticks(30);
    let me = sim.character_of(0).expect("client 0's player");
    // The client types the short name in CS:S's scope: the engine's command.
    submit(&mut sim, Some(0), "scope cs_source; setpos 100 200 300");
    assert!(
        sim.until(300, |s| !s.server.app.world().resource::<Heard>().0.is_empty()),
        "the server hears the command"
    );
    assert_eq!(
        sim.server.app.world().resource::<Heard>().0,
        ["chimera:setpos 100 200 300"]
    );
    sim.ticks(5);
    // It ran the engine's setpos on the client's player (Source units, feet).
    let at = sim.server.app.world().get::<Transform>(me).unwrap().translation;
    assert!(
        (at.x - 100.0 * 0.0254).abs() < 0.05 && (at.z + 200.0 * 0.0254).abs() < 0.05,
        "{at}"
    );
    assert_eq!(
        sim.server.app.world().resource::<OtherSetpos>().0,
        0,
        "not the server scope's setpos"
    );
}

/// The engine's Source names are `chimera:`'s on the wire; a server
/// script written before the move (`cs_source:decalfrequency`) sets the
/// same cvar, and the client gets it by the new full name.
#[test]
fn engine_cvars_replicate_as_chimeras_and_the_old_name_sets_them() {
    let mut sim = joined();
    submit(&mut sim, None, "cs_source:decalfrequency 7");
    assert!(
        sim.until(300, |s| s.clients[0].app.world().resource::<Rules>().engine == 7),
        "the client gets the engine's cvar"
    );
    let c = sim.server.app.world().resource::<Console>();
    assert_eq!(c.cvar("decalfrequency").unwrap().name, "chimera:decalfrequency");
    assert_eq!(c.cvar_scope("cs_source:decalfrequency"), chimera_core::console::CvarScope::Replicated);
    // The client can't set it: the server's.
    submit(&mut sim, Some(0), "chimera:decalfrequency 1");
    sim.ticks(10);
    assert_eq!(sim.clients[0].app.world().resource::<Rules>().engine, 7);
}
