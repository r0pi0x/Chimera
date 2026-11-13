//! The end-of-match results in the minigame's panel (the shell's overlay)
//! whichever HUD is up (the HUD follows the held gun's game): the winner
//! and a line for each player under CS:S's HUD (an AWP in hand) and under
//! Combat Arms' (the M60), through the intermission, gone with the next
//! match. Headless, on the same setup as
//! `hud` (both HUDs' plugins and the shell's switching).

use bevy::prelude::*;
use chimera_core::core::{Damage, DamageKind, Hitgroup};
use chimera_sim::minigame::{self, MatchResults, runtime};
use cs_source::ui::minigame_hud::PanelLine;

use crate::{
    gun_game::held,
    hud::{cs_minigame_lines, hud, party_with_huds, shown, swap_to},
};

/// The match ends with `me` on the last rung killing `other`.
fn win_the_match(sim: &mut chimera_world::harness::Sim, me: Entity, other: Entity) {
    swap_to(sim, me, 9);
    sim.seconds(1.0);
    let at = sim.position(other);
    sim.app.world_mut().write_message(Damage {
        target: other,
        attacker: Some(me),
        amount: 5.0,
        point: at,
        dir: Vec3::NEG_Z,
        hitgroup: Hitgroup::Chest,
        kind: DamageKind::Bullet,
        weapon: None,
        force: Vec3::ZERO,
    });
    sim.seconds(runtime::ROUND_OVER as f64 + 1.0);
}

/// The match-end screen's flag (`MinigameHud::results_shown`), as the
/// shell sets it while its screen is up.
fn set_screen_up(sim: &mut chimera_world::harness::Sim, up: bool) {
    sim.app.world_mut().resource_mut::<chimera_sim::minigame::hud::MinigameHud>().results_shown = up;
    sim.seconds(0.2);
}

#[test]
fn the_results_panel_shows_on_both_huds() {
    let (mut sim, me, other) = party_with_huds();
    sim.seconds(0.5);
    win_the_match(&mut sim, me, other);
    assert!(sim.app.world().get_resource::<MatchResults>().is_some(), "the match is decided");
    // CS:S's HUD: the winner, then each player, in the panel.
    assert_eq!(held(&sim, me), Some(cs_source::game::weapons::AWP));
    assert_eq!(hud(&sim), cs_source::game::GAME);
    let lines: Vec<String> = cs_minigame_lines(&mut sim)
        .into_iter()
        .filter(|(k, _)| *k == PanelLine::Result)
        .map(|(_, t)| t)
        .collect();
    assert_eq!(lines.len(), 3, "the winner and two players: {lines:?}");
    assert!(lines[0].ends_with("wins the match"), "{lines:?}");
    assert!(lines[1].starts_with("1. ") && lines[1].contains("rung 10/10") && lines[1].contains("1 kills"), "{lines:?}");
    assert!(lines[2].starts_with("2. ") && lines[2].contains("rung 1/10") && lines[2].contains("1 deaths"), "{lines:?}");
    // The shell's match-end screen up (`client::match_end`): the whole
    // panel steps aside, title and status lines too (they drew into the
    // screen's winner, seen in a capture).
    set_screen_up(&mut sim, true);
    assert_eq!(cs_minigame_lines(&mut sim), [], "nothing of CS:S's panel over the screen");
    set_screen_up(&mut sim, false);
    assert!(!cs_minigame_lines(&mut sim).is_empty(), "back once it goes");
    // Combat Arms' HUD (the M60 in hand): the same results in its view.
    swap_to(&mut sim, me, 8);
    assert_eq!(hud(&sim), combat_arms::game::GAME);
    assert!(shown(&mut sim, combat_arms::game::GAME));
    let results = |sim: &mut chimera_world::harness::Sim| -> Vec<String> {
        cs_minigame_lines(sim).into_iter().filter(|(k, _)| *k == PanelLine::Result).map(|(_, t)| t).collect()
    };
    assert_eq!(results(&mut sim), lines, "the same results under Combat Arms' HUD");
    set_screen_up(&mut sim, true);
    assert_eq!(cs_minigame_lines(&mut sim), [], "nothing over the screen under Combat Arms' HUD");
    set_screen_up(&mut sim, false);
    // The next match: the results are gone.
    minigame::restart(sim.app.world_mut());
    sim.seconds(0.5);
    assert!(!cs_minigame_lines(&mut sim).is_empty() && results(&mut sim).is_empty());
}
