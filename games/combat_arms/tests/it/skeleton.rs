//! The Combat Arms skeleton (wave 0) on the greybox: a player spawns with
//! the placeholder knife and rifle, walks, fires the rifle (it hurts a
//! target) and switches to the knife. These check the wiring, not Combat
//! Arms' behaviour: the numbers are the placeholder data's.

use crate::common::{active, active_id};
use bevy::prelude::*;
use chimera_core::{
    core::Health,
    game_registry::Games,
    slots::{Loadout, MovementSlot},
};
use chimera_sim::{
    rule_module,
    weapon::{Inventory, Magazine, Weapon},
};
use chimera_world::{
    greybox::{self, GreyboxMapPlugin},
    harness::Sim,
};
use combat_arms::{
    game::{self, inventory, movement, rules, weapons},
    harness::CaSim,
};

fn sim() -> Sim {
    Sim::new(CaSim, GreyboxMapPlugin)
}

/// A Combat Arms player at `at` with the game's movement, settled.
fn player(sim: &mut Sim, at: Vec3) -> Entity {
    let p = sim.spawn_character(at, movement::ID);
    sim.seconds(0.5);
    p
}

fn clip(sim: &Sim, p: Entity) -> u32 {
    let w = active(sim, p).expect("no active weapon");
    sim.app.world().get::<Magazine>(w).unwrap().clip
}

fn health(sim: &Sim, p: Entity) -> f32 {
    sim.app.world().get::<Health>(p).unwrap().current
}

#[test]
fn the_game_registers_itself() {
    let sim = sim();
    let world = sim.app.world();
    assert_eq!(world.resource::<Games>().default_game().map(|g| g.id), Some(game::GAME));
    assert_eq!(world.resource::<Loadout>().movement, movement::ID);
    assert_eq!(
        rule_module::pick(world),
        Some(rules::NO_MODE),
        "the harness plays no mode"
    );
}

#[test]
fn spawns_on_the_greybox_with_knife_and_rifle_drawn() {
    let mut sim = sim();
    let p = player(&mut sim, greybox::SPAWNS[0]);
    assert!(sim.state(p).on_ground, "settled on the greybox's floor");
    assert_eq!(sim.app.world().get::<MovementSlot>(p).unwrap().0, movement::ID);
    let inv = sim.app.world().get::<Inventory>(p).unwrap();
    let ids: Vec<&str> = inv
        .weapons
        .iter()
        .map(|w| sim.app.world().get::<Weapon>(*w).unwrap().id)
        .collect();
    assert_eq!(ids, inventory::STARTING_WEAPONS);
    assert_eq!(active_id(&sim, p), weapons::RIFLE, "the primary is drawn");
}

#[test]
fn walks_forward() {
    let mut sim = sim();
    let p = player(&mut sim, greybox::SPAWNS[0]);
    let start = sim.position(p);
    sim.intent(p).move_axis = Vec2::Y;
    sim.seconds(1.0);
    let moved = sim.position(p) - start;
    // Yaw 0 looks down -Z.
    assert!(moved.z < -2.0, "moved {moved}");
    assert!(sim.velocity(p).xz().length() > 1.0);
}

#[test]
fn the_rifle_fires_and_hurts_a_target() {
    let mut sim = sim();
    let shooter = player(&mut sim, greybox::SPAWNS[0]);
    // A target 5 m straight ahead (yaw 0 looks down -Z), on the same floor.
    let target = player(&mut sim, greybox::SPAWNS[0] - Vec3::Z * 5.0);
    // At the target's chest (hull centre): its head leans to the rifle,
    // off the line of the eyes (characters.md 11).
    {
        let d = sim.position(target) - (sim.position(shooter) + sim.state(shooter).eye_offset);
        sim.intent(shooter).pitch = d.y.atan2(d.xz().length());
    }
    let before = health(&sim, target);
    let full = clip(&sim, shooter);
    sim.intent(shooter).fire = true;
    sim.seconds(0.5);
    sim.intent(shooter).fire = false;
    sim.ticks(2);
    let fired = full - clip(&sim, shooter);
    assert!(
        fired >= 2,
        "an automatic rifle fires more than once in 0.5 s: {fired} rounds"
    );
    assert!(health(&sim, target) < before, "the target took no damage");
}

#[test]
fn switches_to_the_knife_and_back() {
    let mut sim = sim();
    let p = player(&mut sim, greybox::SPAWNS[0]);
    let knife_slot = sim
        .app
        .world()
        .get::<Inventory>(p)
        .unwrap()
        .weapons
        .iter()
        .map(|w| sim.app.world().get::<Weapon>(*w).unwrap())
        .find(|w| w.id == inventory::KNIFE)
        .unwrap()
        .slot;
    sim.intent(p).select = Some(knife_slot);
    sim.ticks(2);
    sim.intent(p).select = None;
    sim.ticks(1);
    assert_eq!(active_id(&sim, p), inventory::KNIFE);
    // Back with the last-weapon key.
    sim.intent(p).last_weapon = true;
    sim.ticks(2);
    sim.intent(p).last_weapon = false;
    sim.ticks(1);
    assert_eq!(active_id(&sim, p), weapons::RIFLE);
}
