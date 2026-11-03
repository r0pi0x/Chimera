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

/// A player as Source's log writes it, with HL2:DM's team name:
/// `"Name<userid><uniqueid><Team>"` (the gameplay's `LogSubjects`).
pub fn subject(world: &World, e: Entity, team: &str) -> String {
    let resolve = crate::gameplay::active_hook::<crate::game_log::LogSubjects>(world, |g| g.rules)
        .unwrap_or_default()
        .0;
    let s = resolve(world, e).unwrap_or_else(|| crate::game_log::LogSubject {
        name: "unknown".into(),
        userid: e.index().index() as u64,
        unique: String::new(),
    });
    format!("\"{}<{}><{}><{}>\"", s.name, s.userid, s.unique, team)
}

/// A log line of text.
pub fn log_text(world: &mut World, text: String) {
    log(world, vec![LogPart::Text(text)]);
}
