//! The console's scope in the party (docs/plans/active/console-namespaces.md):
//! short names mean the names of the game of the gun in hand (the local
//! player's `GameIdentity`), then Chimera's; `scope <game>` pins one;
//! `scope auto` follows the gun again.

use bevy::prelude::*;
use chimera_core::{
    console::{Console, Resolved, resource_cvar},
    core::LocalPlayer,
};
use chimera_world::harness::Sim;

use crate::gun_game::{held, party, soldier};
use crate::switches::set_rung;

/// One short name in each game.
#[derive(Resource, Default)]
struct Both(u8, u8);

fn resolve(sim: &Sim, name: &str) -> Resolved {
    sim.app.world().resource::<Console>().resolve(name)
}

fn is(full: &str) -> Resolved {
    Resolved::Found(full.into())
}

fn submit(sim: &mut Sim, line: &str) -> Vec<String> {
    let start = sim.app.world().resource::<Console>().output.len();
    sim.app.world_mut().resource_mut::<Console>().submit(line);
    sim.ticks(1);
    sim.app.world().resource::<Console>().output[start..]
        .iter()
        .map(|l| l.text.clone())
        .collect()
}

/// The first rung whose gun is `game`'s, with `e` on it.
fn to_rung_of(sim: &mut Sim, e: Entity, game: &str) {
    for r in 0..12 {
        set_rung(sim, e, r);
        sim.seconds(0.5);
        if held(sim, e).is_some_and(|id| id.starts_with(&format!("{game}:"))) {
            return;
        }
    }
    panic!("no rung with a {game} gun");
}

#[test]
fn short_names_follow_the_gun_in_hand() {
    let mut sim = party();
    sim.app.init_resource::<Both>();
    resource_cvar::<Both, u8>(&mut sim.app, "cs_source:party_test", "CS:S's", |b| &mut b.0);
    resource_cvar::<Both, u8>(&mut sim.app, "combat_arms:party_test", "Combat Arms'", |b| &mut b.1);
    let me = soldier(&mut sim);
    let _other = soldier(&mut sim);
    sim.app.world_mut().entity_mut(me).insert(LocalPlayer);
    sim.seconds(1.5);

    to_rung_of(&mut sim, me, "cs_source");
    assert_eq!(resolve(&sim, "party_test"), is("cs_source:party_test"));
    submit(&mut sim, "party_test 4");
    assert_eq!(sim.app.world().resource::<Both>().0, 4);

    to_rung_of(&mut sim, me, "combat_arms");
    assert_eq!(resolve(&sim, "party_test"), is("combat_arms:party_test"));
    submit(&mut sim, "party_test 6");
    assert_eq!(sim.app.world().resource::<Both>().1, 6);
    assert!(submit(&mut sim, "scope")[0].contains("combat_arms (the game played)"));

    // Pinned, it stays whatever the gun.
    submit(&mut sim, "scope css");
    to_rung_of(&mut sim, me, "cs_source");
    to_rung_of(&mut sim, me, "combat_arms");
    assert_eq!(resolve(&sim, "party_test"), is("cs_source:party_test"));
    // Back to following the gun.
    submit(&mut sim, "scope auto");
    assert_eq!(resolve(&sim, "party_test"), is("combat_arms:party_test"));
    // Full names and aliases whatever the scope.
    assert_eq!(resolve(&sim, "css:party_test"), is("cs_source:party_test"));
    assert_eq!(resolve(&sim, "ca:party_test"), is("combat_arms:party_test"));
}

/// The engine's Source names are Chimera's: the same cvar whatever the
/// gun in hand, and the old `cs_source:` spelling (saved configs, binds)
/// still reaches it; CS:S's tuned values stay in its namespace.
#[test]
fn engine_names_mean_the_same_in_every_game() {
    let mut sim = party();
    sim.app.init_resource::<Both>();
    // Registered bare, as the apps do: one engine name, one game-tuned.
    resource_cvar::<Both, u8>(&mut sim.app, "fps_max", "an engine name", |b| &mut b.0);
    resource_cvar::<Both, u8>(&mut sim.app, "sv_gravity", "a tuned value", |b| &mut b.1);
    let me = soldier(&mut sim);
    sim.app.world_mut().entity_mut(me).insert(LocalPlayer);
    sim.seconds(1.5);
    for game in ["cs_source", "combat_arms"] {
        to_rung_of(&mut sim, me, game);
        assert_eq!(resolve(&sim, "fps_max"), is("chimera:fps_max"), "{game}");
    }
    assert_eq!(resolve(&sim, "cs_source:fps_max"), is("chimera:fps_max"));
    assert_eq!(resolve(&sim, "css:fps_max"), is("chimera:fps_max"));
    assert_eq!(resolve(&sim, "sv_gravity"), is("cs_source:sv_gravity"), "game-tuned stays CS:S's");
    assert_eq!(resolve(&sim, "chimera:sv_gravity"), Resolved::Unknown);
}
