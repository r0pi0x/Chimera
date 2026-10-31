//! The server on the greybox (`+map greybox`): the command line's
//! minigame (the gun game without `+minigame`) takes the gameplay's rules
//! at the first frame; two soldiers make the match live, each on the
//! ladder's first rung; a kill moves the killer up.

use bevy::prelude::*;
use chimera_core::gameplay::Gameplay;
use chimera_sim::{
    minigame::{ActiveMinigame, MINIGAME, ladder::Rung},
    mode::{Match, Phase},
    testkit,
};

use crate::support::*;

#[test]
fn the_server_plays_the_gun_game() {
    let mut sim = server(&["+map", "greybox"]).expect("the greybox loads");
    assert_eq!(sim.app.world().resource::<Gameplay>().rules, MINIGAME);
    let id = sim.app.world().resource::<ActiveMinigame>().def.id.clone();
    assert_eq!(id, party_kit::DEFAULT_MINIGAME);
    let a = soldier(&mut sim, Vec3::new(0.0, 1.0, 0.0));
    let b = soldier(&mut sim, Vec3::new(2.0, 1.0, 0.0));
    sim.seconds(1.0);
    assert_eq!(sim.app.world().resource::<Match>().phase, Phase::Live);
    assert_eq!(testkit::active_id(sim.app.world(), a), Some(cs_source::game::weapons::GLOCK));
    // Protection (2 s) over, b 4 m in front of a.
    sim.seconds(1.5);
    // On the greybox's open middle (a spawn spot may be near a ledge).
    let open = Vec3::new(0.0, 1.0, 0.0);
    place(&mut sim, a, open);
    place(&mut sim, b, open + Vec3::new(0.0, 0.0, -4.0));
    sim.seconds(0.5);
    assert!(shoot_until_dead(&mut sim, a, b, 8.0), "the Glock kills");
    assert_eq!(sim.app.world().get::<Rung>(a).map(|r| r.0), Some(1));
}
