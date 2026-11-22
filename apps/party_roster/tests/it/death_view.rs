//! The gun game's death view and respawn countdown (backlog: "the death
//! view fights the 2 s respawn"): a player holding a CS:S gun, killed,
//! gets CS:S's death cam and freeze cam fitted into the minigame's 2 s
//! (the picture frozen well before the respawn, still up just before
//! it), the shell's countdown says when it comes back ("Respawning in
//! 2", then 1), and both end with the respawn. Headless, on the party's
//! simulation with both HUDs (`hud::party_with_huds_and`), the shell's
//! spectator state and its countdown.

use bevy::prelude::*;
use chimera_client::client::{
    respawn_countdown::{RespawnCountdownPlugin, RespawnCountdownRoot, RespawnCountdownText},
    spectate::{SpecPhase, SpectateStatePlugin, Spectator},
};
use chimera_core::core::{Dead, RespawnDue};
use chimera_world::harness::Sim;

use crate::{
    gun_game::{alive, held, line_up, shoot_until_dead},
    hud::{hud, party_with_huds_and},
};

fn countdown(sim: &mut Sim) -> Option<String> {
    let w = sim.app.world_mut();
    let v = *w.query_filtered::<&Visibility, With<RespawnCountdownRoot>>().single(w).unwrap();
    let t = w.query_filtered::<&Text, With<RespawnCountdownText>>().single(w).unwrap().0.clone();
    (v != Visibility::Hidden).then_some(t)
}

fn phase(sim: &Sim) -> SpecPhase {
    sim.app.world().resource::<Spectator>().phase
}

#[test]
fn a_cs_gun_death_freezes_on_the_killer_and_counts_down_to_the_respawn() {
    let (mut sim, me, other) = party_with_huds_and(|app| {
        app.add_plugins((SpectateStatePlugin, RespawnCountdownPlugin));
    });
    assert_eq!(held(&sim, me), Some(cs_source::game::weapons::GLOCK));
    assert_eq!(hud(&sim), cs_source::game::GAME, "CS:S's identity");
    line_up(&mut sim, other, me, 3.0);
    assert!(shoot_until_dead(&mut sim, other, me, 5.0), "killed");
    let since = sim.app.world().get::<Dead>(me).unwrap().since;
    sim.ticks(2);
    let due = sim.app.world().resource::<RespawnDue>().0.expect("the gun game's timed respawn");
    assert!((due.at - since - 2.0).abs() < 1e-6, "the definition's 2 s: {due:?}");
    assert_eq!(countdown(&mut sim).as_deref(), Some("Respawning in 2"));
    assert!(matches!(phase(&sim), SpecPhase::DeathCam { .. }), "{:?}", phase(&sim));
    // The freeze cam on the killer within the first second, its picture
    // held to the respawn.
    sim.seconds(0.9);
    assert!(
        matches!(phase(&sim), SpecPhase::FreezeCam { killer, .. } if killer == other),
        "{:?}",
        phase(&sim)
    );
    assert_eq!(countdown(&mut sim).as_deref(), Some("Respawning in 2"));
    sim.seconds(0.4);
    assert_eq!(countdown(&mut sim).as_deref(), Some("Respawning in 1"));
    sim.seconds(0.5);
    assert!(!alive(&sim, me), "still dead just before the 2 s");
    assert!(matches!(phase(&sim), SpecPhase::FreezeCam { .. }), "{:?}", phase(&sim));
    // Back: no death view, no countdown.
    sim.seconds(0.3);
    assert!(alive(&sim, me), "respawned at 2 s");
    sim.ticks(2);
    assert_eq!(phase(&sim), SpecPhase::Alive);
    assert_eq!(countdown(&mut sim), None);
    assert_eq!(sim.app.world().resource::<RespawnDue>().0, None);
}
