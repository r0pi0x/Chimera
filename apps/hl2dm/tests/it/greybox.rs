//! The app's simulation on the greybox (no install): a player on HL2:DM's
//! movement with its loadout (the SMG1 in hand) stands, runs and shoots.

use bevy::prelude::*;
use chimera_sim::weapon::{Inventory, Magazine};
use chimera_world::{
    greybox::{self, GreyboxMapPlugin},
    harness::Sim,
};
use hl2dm::game::movement;

#[test]
fn a_player_plays_the_greybox() {
    let mut sim = Sim::new(hl2dm_app::SimPlugin, GreyboxMapPlugin);
    let p = sim.spawn_character(greybox::SPAWNS[0], movement::ID);
    sim.seconds(1.5);
    assert!(sim.state(p).on_ground, "stands on the greybox");
    let start = sim.position(p);
    sim.intent(p).move_axis = Vec2::Y;
    sim.seconds(0.5);
    assert!(sim.position(p).distance(start) > 1.0, "runs");
    sim.intent(p).move_axis = Vec2::ZERO;
    sim.intent(p).fire = true;
    sim.seconds(0.2);
    let w = sim.app.world();
    let gun = w.get::<Inventory>(p).and_then(|i| i.active).expect("a weapon in hand");
    // The spawn's loadout puts the SMG1 in hand (specs/hl2dm/weapons.md 3).
    // 0.2 s held: the SMG1's first rounds (one every 0.075 s).
    let clip = w.get::<Magazine>(gun).unwrap().clip;
    assert!((41..=43).contains(&clip), "a few rounds of the SMG1's 45: {clip}");
}
