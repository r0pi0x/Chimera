//! DoD:S's bodies in the sandbox (docs/plans/active/dods.md, M0): the
//! player given DoD:S's Colt (`rung`) becomes DoD:S's (its identity and
//! `dods:movement`) and stands with its eye 58 units over its feet
//! (specs/dods/movement.md, "Constants"); a practice target wearing a
//! DoD:S class body (`target add dods:axis_mg`) holds DoD:S's gun, moves
//! on its movement and wears that class's model by name. On the party's
//! simulation, headless.

use bevy::prelude::*;
use chimera_core::{
    console::Console,
    core::{LocalPlayer, MovementState},
    slots::{GameIdentity, MovementSlot},
};
use chimera_sim::minigame::{self, targets::Target};
use chimera_world::{greybox, harness::Sim, map::BodyName};
use combat_arms::game::movement;
use ultimate_gun_game::sandbox::SANDBOX;

use crate::parts::ugg;

const COLT: &str = "dods:weapon_colt";
const UNIT: f32 = 0.0254;

/// The sandbox on the greybox with the local player `me` in it.
fn play() -> (Sim, Entity) {
    let mut sim = ugg();
    let w = sim.app.world_mut();
    greybox::unload(w);
    greybox::select(w, "greybox");
    greybox::respawn(w);
    sim.ticks(2);
    ultimate_gun_game::sandbox::generate(sim.app.world_mut(), "combat_arms").expect("the definition");
    minigame::start(sim.app.world_mut(), SANDBOX).expect("the sandbox starts");
    let me = sim.spawn_character(Vec3::new(0.0, 1.0, 0.0), movement::ID);
    sim.app.world_mut().entity_mut(me).insert(LocalPlayer);
    sim.seconds(1.5);
    (sim, me)
}

fn run(sim: &mut Sim, line: &str) -> String {
    let before = sim.app.world().resource::<Console>().output.len();
    sim.app.world_mut().resource_mut::<Console>().submit(line.to_string());
    sim.ticks(3);
    sim.app.world().resource::<Console>().output[before..].iter().map(|l| l.text.clone()).collect::<Vec<_>>().join("\n")
}

fn movement_of(sim: &Sim, e: Entity) -> Option<&'static str> {
    sim.app.world().get::<MovementSlot>(e).map(|m| m.0)
}

fn identity_of(sim: &Sim, e: Entity) -> Option<String> {
    sim.app.world().get::<GameIdentity>(e).map(|i| i.0.to_string())
}

#[test]
fn the_colt_makes_the_player_dods_with_its_eye_at_58() {
    let (mut sim, me) = play();
    let said = run(&mut sim, &format!("rung {COLT}"));
    sim.seconds(1.5);
    assert_eq!(movement_of(&sim, me), Some("dods:movement"), "{said}");
    assert_eq!(identity_of(&sim, me).as_deref(), Some("dods"));
    assert!(sim.state(me).on_ground);
    // The eye over the feet: the origin is the standing box's middle (36).
    let ms = sim.app.world().get::<MovementState>(me).cloned().unwrap();
    let eye = 36.0 + ms.eye_offset.y / UNIT;
    assert!((eye - 58.0).abs() < 0.5, "eye {eye} units over the feet");
}

#[test]
fn a_dods_class_body_stands_as_a_target() {
    let (mut sim, _) = play();
    let said = run(&mut sim, "target add dods:axis_mg 5");
    sim.seconds(1.5);
    let mut q = sim.app.world_mut().query_filtered::<Entity, With<Target>>();
    let targets: Vec<Entity> = q.iter(sim.app.world()).collect();
    assert_eq!(targets.len(), 1, "{said}");
    let t = targets[0];
    assert_eq!(movement_of(&sim, t), Some("dods:movement"), "{said}");
    assert_eq!(identity_of(&sim, t).as_deref(), Some("dods"));
    let name = sim.app.world().get::<BodyName>(t).map(|n| n.0.clone());
    assert_eq!(name.as_deref(), Some("models/player/german_mg.mdl"));
    let ms = sim.app.world().get::<MovementState>(t).cloned().unwrap();
    assert!((36.0 + ms.eye_offset.y / UNIT - 58.0).abs() < 0.5, "the target's eye");
}
