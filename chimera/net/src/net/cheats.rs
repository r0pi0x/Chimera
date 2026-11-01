//! Cheat commands in a network game (Source's FCVAR_CHEAT commands;
//! `console::CHEATS`): a network client's `noclip`, `god`, `setpos`, ...
//! go to its server (`ClientCommand`, the command's name in full:
//! `chimera:noclip`), which runs them for that client's
//! player (`console::CommandPlayer`) when its own `sv_cheats` is 1, and
//! sends back what they printed (`ConsoleText`): Source's way, where the
//! client sends cheats on and the server decides. A player's movement
//! implementation is in its state from the server (`OwnState::movement`),
//! so a client's prediction follows the server's `noclip`. `noclip`
//! remembers the movement it left (`movement::noclip::NoclipBack`), which
//! a game's spawn goes back to (`noclip::end_toggled`).
//!
//! `kill` (`PLAYER_COMMANDS`) goes the same way but needs no `sv_cheats`:
//! the server kills the client's player.
//!
//! Also the player commands both a game and a dedicated server run for a
//! player: `noclip`, `god`, `kill`, `setpos`, `setang` (the game's client adds
//! the rest, `client::console`).

use bevy::prelude::*;
use bevy_replicon::prelude::*;
use serde::{Deserialize, Serialize};

use super::server::Player;
use crate::{
    console::{
        CommandPlayer, Console, ConsoleAppExt, Level, PLAYER_COMMANDS, cheats_allowed, command_player, execute,
        in_table, is_cheat, parse,
    },
    core::{MovementState, NetRole, Velocity},
    movement::noclip::NoclipBack,
    slots::{Loadout, MovementSlot, set_movement},
};

/// Client -> server: run this console line for my player (a cheat).
#[derive(Message, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct ClientCommand {
    pub line: String,
}

/// Server -> a client: what its command printed, for its console.
#[derive(Message, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct ConsoleText {
    pub lines: Vec<(String, bool)>,
}

/// Longest command line a client may send.
const MAX_LINE: usize = 256;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        PreUpdate,
        receive_commands
            .after(ServerSystems::Receive)
            .run_if(in_state(ServerState::Running)),
    )
    .add_systems(
        PostUpdate,
        send_commands
            .before(ClientSystems::Send)
            .run_if(resource_equals(NetRole::Client)),
    )
    .add_systems(
        PreUpdate,
        print_replies
            .after(ClientSystems::Receive)
            .run_if(resource_equals(NetRole::Client)),
    );
    player_commands(app);
}

/// A client's cheats, to its server.
fn send_commands(mut console: ResMut<Console>, mut out: MessageWriter<ClientCommand>) {
    for line in std::mem::take(&mut console.to_server) {
        out.write(ClientCommand { line });
    }
}

/// What the server's run of our commands printed.
fn print_replies(mut replies: MessageReader<ConsoleText>, mut console: ResMut<Console>) {
    for r in replies.read() {
        for (text, error) in &r.lines {
            console.print(if *error { Level::Error } else { Level::Info }, text.clone());
        }
    }
}

/// Clients' cheat commands: run for their player when `sv_cheats` is 1
/// (only commands in `CHEATS`; anything else a client sends is dropped).
fn receive_commands(mut requests: MessageReader<FromClient<ClientCommand>>, mut commands: Commands) {
    for r in requests.read() {
        let Some(client) = r.client_id.entity() else { continue };
        let line = r.message.line.clone();
        if line.len() > MAX_LINE {
            continue;
        }
        commands.queue(move |w: &mut World| run_for(w, client, &line));
    }
}

/// Run `line` for `client`'s player and send it what came of it.
pub fn run_for(world: &mut World, client: Entity, line: &str) {
    let Some(player) = world.get::<Player>(client).map(|p| p.character) else {
        return;
    };
    let mut lines = Vec::new();
    for words in parse(line) {
        let Some(name) = words.first().map(|w| w.to_lowercase()) else {
            continue;
        };
        // Clients send full names (`Console::wire_line`); a short one
        // resolves in the server's scope.
        let console = world.resource::<Console>();
        let cheat = is_cheat(console, &name, &words[1..]);
        if !cheat && !in_table(console, PLAYER_COMMANDS, &name) {
            continue;
        }
        if cheat && !cheats_allowed(world) {
            // Source's words.
            lines.push((
                format!(
                    "Can't use cheat command {} in multiplayer, unless the server has sv_cheats set to 1.",
                    crate::console::short_name(&name)
                ),
                true,
            ));
            continue;
        }
        let who = world.get::<Player>(client).map(|p| p.name.clone()).unwrap_or_default();
        info!("{who}: {}", words.join(" "));
        let before = world.resource::<Console>().printed;
        world.insert_resource(CommandPlayer(player));
        execute(world, &words, 0);
        world.remove_resource::<CommandPlayer>();
        let console = world.resource::<Console>();
        let n = (console.printed - before) as usize;
        let out = &console.output;
        lines.extend(
            out[out.len().saturating_sub(n)..]
                .iter()
                .map(|l| (l.text.clone(), l.level == Level::Error)),
        );
    }
    if !lines.is_empty() {
        world.write_message(ToClients {
            targets: SendTargets::Single(ClientId::Client(client)),
            message: ConsoleText { lines },
        });
    }
}

/// The eye and the feet relative to a character's origin
/// (`MovementState::eye_offset`, `hull_min`); see `setpos`.
fn eye_and_feet(w: &World, p: Entity) -> (Vec3, Vec3) {
    w.get::<MovementState>(p)
        .map_or((Vec3::ZERO, Vec3::ZERO), |s| (s.eye_offset, Vec3::Y * s.hull_min.y))
}

/// `noclip`, `god`, `setpos`, `setang` for the player a command acts on
/// (`console::command_player`): the local player, or on a server a
/// client's.
pub fn player_commands(app: &mut App) {
    app.console_command(
        "noclip",
        "Fly through walls (toggles back to the movement you had).",
        |w, _| {
            let p = command_player(w)?;
            let current = w.get::<MovementSlot>(p).map(|s| s.0).ok_or("no movement")?;
            let next = if current == crate::movement::noclip::ID {
                w.entity_mut(p)
                    .take::<NoclipBack>()
                    .map(|b| b.0)
                    .or_else(|| w.get_resource::<Loadout>().map(|l| l.movement))
                    .filter(|m| *m != crate::movement::noclip::ID)
                    .unwrap_or(crate::movement::placeholder::ID)
            } else {
                w.entity_mut(p).insert(NoclipBack(current));
                crate::movement::noclip::ID
            };
            set_movement(p, next).apply(w);
            Ok(Some(format!(
                "noclip {}",
                if next == crate::movement::noclip::ID { "ON" } else { "OFF" }
            )))
        },
    )
    .console_command(
        "setpos",
        "setpos <x> <y> <z>: put your feet there (CS:S units; Source's origin: getpos prints the eye, 64 higher standing).",
        |w, a| {
            let [x, y, z] = a else {
                return Err("setpos <x> <y> <z>".into());
            };
            let n = |s: &String| s.parse::<f32>().map_err(|_| format!("bad number \"{s}\""));
            let v = Vec3::new(n(x)?, n(z)?, -n(y)?) * 0.0254;
            let p = command_player(w)?;
            let origin = v - eye_and_feet(w, p).1;
            w.get_mut::<Transform>(p).ok_or("no transform")?.translation = origin;
            w.get_mut::<Velocity>(p).ok_or("no velocity")?.0 = Vec3::ZERO;
            Ok(None)
        },
    )
    .console_command(
        "setang",
        "setang <pitch> <yaw>: look there (degrees, CS:S convention).",
        |w, a| {
            let [pitch, yaw, ..] = a else {
                return Err("setang <pitch> <yaw>".into());
            };
            let n = |s: &String| s.parse::<f32>().map_err(|_| format!("bad number \"{s}\""));
            let (pitch, yaw) = (n(pitch)?, n(yaw)?);
            let p = command_player(w)?;
            let mut i = w.get_mut::<crate::core::Intent>(p).ok_or("no intent")?;
            i.pitch = -pitch.to_radians();
            i.yaw = (yaw - 90.0).to_radians();
            Ok(None)
        },
    )
    .console_command(
        "kill",
        "Kill yourself (CS:S's suicide: you die where you stand, which can end the round).",
        |w, _| {
            use crate::core::{Damage, DamageKind, Dead, Health, Hitgroup};
            let p = command_player(w)?;
            if w.get::<Dead>(p).is_some() {
                return Ok(None);
            }
            let t = *w.get::<Transform>(p).ok_or("no transform")?;
            let _ = w.get::<Health>(p).ok_or("no health")?;
            // God mode would refuse the damage; a suicide goes through it.
            w.entity_mut(p).remove::<crate::core::God>();
            w.write_message(Damage {
                force: Vec3::ZERO,
                target: p,
                attacker: Some(p),
                amount: 1000.0,
                point: t.translation,
                dir: Vec3::NEG_Y,
                hitgroup: Hitgroup::Generic,
                kind: DamageKind::Generic,
                weapon: None,
            });
            Ok(None)
        },
    )
    .console_command("god", "Toggle taking no damage.", |w, _| {
        let p = command_player(w)?;
        let on = w.get::<crate::core::God>(p).is_none();
        if on {
            w.entity_mut(p).insert(crate::core::God);
        } else {
            w.entity_mut(p).remove::<crate::core::God>();
        }
        Ok(Some(format!("godmode {}", if on { "ON" } else { "OFF" })))
    });
}
