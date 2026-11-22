//! A party whose first soldier is the local player, read from the keyboard.

#![allow(dead_code, unused_imports)]

use crate::play::*;
use bevy::{
    input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll},
    prelude::*,
    window::{CursorGrabMode, CursorOptions},
};
use chimera_client::client::{
    input::{FreeLook, MouseSettings, WheelJump, apply_wheel_jump, write_local_intent},
    view::FreeCam,
};
use chimera_core::{console::Console, core::LocalPlayer};
use chimera_world::harness::Sim;
use combat_arms::game::movement::CaMovement;
use party_roster::controls::{self, Action, ControlsPage, ControlsPlugin};
use crate::play::{party, place, soldier};
use crate::play::set_rung;

/// A party whose first soldier is the local player, read from the keyboard
/// as the client reads it (`write_local_intent`, the wheel's jumps).
pub(crate) fn local_party() -> (Sim, Entity) {
    local_party_with(party())
}

/// `local_party` on `sim` (the party with a test's plugins too).
pub(crate) fn local_party_with(mut sim: Sim) -> (Sim, Entity) {
    let a = soldier(&mut sim);
    let _b = soldier(&mut sim);
    let app = &mut sim.app;
    app.init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<ButtonInput<MouseButton>>()
        .init_resource::<AccumulatedMouseMotion>()
        .init_resource::<AccumulatedMouseScroll>()
        .init_resource::<MouseSettings>()
        .init_resource::<WheelJump>()
        .init_resource::<FreeLook>()
        .init_resource::<FreeCam>()
        .add_systems(Update, write_local_intent)
        .add_systems(FixedPreUpdate, apply_wheel_jump);
    app.world_mut().spawn(CursorOptions {
        grab_mode: CursorGrabMode::Locked,
        ..default()
    });
    app.world_mut().entity_mut(a).insert(LocalPlayer);
    sim.seconds(1.5);
    (sim, a)
}
