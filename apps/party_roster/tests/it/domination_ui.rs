//! CS:S's domination parts stay out of the party apps' HUD: a party
//! player killed by someone who dominates them (the kill's `KillMark` and
//! the `Relationships` both say so) gets the plain "KILLED YOU" freeze
//! panel, not "IS YOUR NEW NEMESIS" with CS:S's icon; in CS:S's own app
//! (`DominationUi` left at its default) the same death gives the nemesis
//! case. Headless: the death is remembered (`MyDeath`) and the panel's
//! case read by `case_for`, as the freeze system does.

use bevy::prelude::*;
use chimera_core::core::{Damage, Died, LocalPlayer};
use chimera_sim::dominations::{KillMark, Relationships};
use cs_source_ui::freeze_cam::{DominationUi, FreezeCase, FreezeCamPlugin, MyDeath, case_for};

use crate::hud::party_with_huds_and;

/// The local player killed by `other`, a new domination; the panel's case.
fn case_after_domination_kill(sim: &mut chimera_world::harness::Sim, me: Entity, other: Entity) -> FreezeCase {
    let w = sim.app.world_mut();
    w.entity_mut(me).insert((LocalPlayer, Relationships { dominating: vec![], nemeses: vec![other] }));
    w.write_message(KillMark { victim: me, killer: other, dominated: true, revenge: false });
    w.write_message(Died {
        entity: me,
        attacker: Some(other),
        damage: Damage {
            force: Vec3::ZERO,
            target: me,
            attacker: Some(other),
            amount: 100.0,
            point: Vec3::ZERO,
            dir: Vec3::X,
            hitgroup: chimera_core::core::Hitgroup::Chest,
            kind: default(),
            weapon: None,
        },
    });
    sim.ticks(2);
    let w = sim.app.world();
    assert_eq!(w.resource::<MyDeath>().0, Some((other, true, false)), "the death is remembered");
    let still = w.get::<Relationships>(me).is_some_and(|r| r.nemeses.contains(&other));
    case_for(*w.resource::<DominationUi>(), w.resource::<MyDeath>().0, other, still)
}

fn sim(party_app: bool) -> (chimera_world::harness::Sim, Entity, Entity) {
    party_with_huds_and(|app| {
        app.init_asset::<bevy::shader::Shader>().init_asset::<Image>();
        app.init_resource::<chimera_client::client::spectate::Spectator>();
        if party_app {
            // What the party client runs for CS:S's front end.
            cs_source_ui::party::client_app(app);
        }
        app.add_plugins(FreezeCamPlugin);
        app.world_mut().spawn(Window::default());
    })
}

#[test]
fn a_party_app_shows_the_plain_freeze_panel_for_a_domination_kill() {
    let (mut sim, me, other) = sim(true);
    assert_eq!(*sim.app.world().resource::<DominationUi>(), DominationUi(false));
    assert_eq!(case_after_domination_kill(&mut sim, me, other), FreezeCase::Killed);
    assert_eq!(FreezeCase::Killed.icon(), None, "no domination icon");
}

#[test]
fn cs_source_itself_shows_the_nemesis_case() {
    let (mut sim, me, other) = sim(false);
    assert_eq!(*sim.app.world().resource::<DominationUi>(), DominationUi(true));
    assert_eq!(case_after_domination_kill(&mut sim, me, other), FreezeCase::NewNemesis);
}
