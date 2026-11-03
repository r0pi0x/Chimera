//! HL2:DM's game events and server log lines (specs/hl2dm/rules.md "Game
//! events", "Server log lines"; maps_entities.md 1's `ammo_pickup`): what
//! its game's parts (`game::rules`, `game::pickups`) tell plugins, tests
//! and admins, by Source's names (`game_log`).

use bevy::prelude::*;

use crate::game_log::{EventValue, GameEvent, LogLine, LogPart};

/// `ammo_pickup`: `who` took `amount` rounds of `ammo`, now carrying
/// `total`.
pub fn ammo_pickup(world: &mut World, who: Entity, ammo: &str, amount: u32, total: u32) {
    world.write_message(
        GameEvent::of("ammo_pickup", who)
            .with("ammo", EventValue::Str(ammo.to_string()))
            .with("amount", EventValue::Int(amount as i64))
            .with("total", EventValue::Int(total as i64)),
    );
}

/// A game event.
pub fn event(world: &mut World, event: GameEvent) {
    world.write_message(event);
}

/// A log line of text pieces.
pub fn log(world: &mut World, parts: Vec<LogPart>) {
    world.write_message(LogLine(parts));
}
