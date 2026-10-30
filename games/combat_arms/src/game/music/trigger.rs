//! Map music triggers (music_ambience.md 5): the server's `BGMTrigger`
//! objects. `ON` (when the object's `GameType` is the running mode and
//! it names a file) marks it on and tells every client to start the
//! music, `OFF` marks it off and tells them to stop it; a client that
//! comes in later gets the first trigger that is on and looping, as the
//! server takes its join (`late_join`, an observer of its `Player`). The
//! client applies a start straight on the player (the menu layer's name
//! is bypassed): a cross-fade from what plays when the fade is above 0.

use std::collections::BTreeMap;

use bevy::prelude::*;
use bevy_replicon::prelude::*;
use serde::{Deserialize, Serialize};

use super::super::{
    chat::{self, Target},
    objects::{commands::ObjId, runtime::Props},
    rules::current_mode,
    world::SPAWN_GAME_TYPE,
};
use super::{data::normalize, director::CaMusic};
use crate::net::server::Player;

/// Server to every client: start this music.
#[derive(Message, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct CaMusicOn {
    pub looping: bool,
    /// Seconds.
    pub fade: f32,
    /// The path below `Game\`.
    pub file: String,
}

/// Server to every client: stop the music.
#[derive(Message, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct CaMusicOff;

/// A trigger as the server holds it.
#[derive(Clone, Debug, PartialEq)]
pub struct TriggerState {
    pub on: bool,
    pub msg: CaMusicOn,
}

/// The server's triggers that were switched on or off, by object.
#[derive(Resource, Default, Debug)]
pub struct CaMapMusic {
    pub triggers: BTreeMap<ObjId, TriggerState>,
}

impl CaMapMusic {
    /// The trigger a joining client gets: the first that is on and looping
    /// (a non-looping track still playing isn't sent).
    pub fn for_joiner(&self) -> Option<&CaMusicOn> {
        self.triggers.values().find(|t| t.on && t.msg.looping).map(|t| &t.msg)
    }
}

/// The `GameType` name of the running mode (its spawn family).
pub fn game_type(world: &World) -> Option<&'static str> {
    current_mode(world).map(|m| m.spawns.family.unwrap_or(SPAWN_GAME_TYPE))
}

/// A `BGMTrigger` got `ON` (`on`) or `OFF`: the server's side.
pub fn on_message(world: &mut World, id: ObjId, on: bool, props: &Props) {
    let matches = game_type(world).is_some_and(|g| g.eq_ignore_ascii_case(props.string("GameType").trim()));
    if !matches {
        return;
    }
    let file = props.string("FileName");
    let msg = CaMusicOn {
        looping: props.bool_or("Loop", false),
        fade: props.real_or(
            "FadingTime",
            super::data::MusicData::built_in().trigger_fade_default as f32,
        ),
        file: file.trim().to_string(),
    };
    if on {
        if msg.file.is_empty() {
            return;
        }
        world.resource_mut::<CaMapMusic>().triggers.insert(
            id,
            TriggerState {
                on: true,
                msg: msg.clone(),
            },
        );
        broadcast(world, msg);
    } else {
        world
            .resource_mut::<CaMapMusic>()
            .triggers
            .insert(id, TriggerState { on: false, msg });
        for (target, _) in chat::readers(world) {
            chat::send(world, target, CaMusicOff);
        }
    }
}

fn broadcast(world: &mut World, msg: CaMusicOn) {
    for (target, _) in chat::readers(world) {
        chat::send(world, target, msg.clone());
    }
}

/// A client came in (an observer of its `Player` being put on, where the
/// server takes the join, on the authoritative side): it gets the one
/// trigger that is on and looping.
pub fn late_join(
    add: On<Add<Player>>,
    music: Option<Res<CaMapMusic>>,
    role: Option<Res<crate::core::NetRole>>,
    playback: Option<Res<crate::core::Playback>>,
    mut commands: Commands,
) {
    if !crate::core::authoritative(role, playback) {
        return;
    }
    let Some(msg) = music.and_then(|m| m.for_joiner().cloned()) else {
        return;
    };
    let client = add.entity;
    commands.queue(move |world: &mut World| chat::send(world, Target::Client(ClientId::Client(client)), msg));
}

/// A map load clears the triggers (a new map's objects are new).
pub fn clear(world: &mut World) {
    world.resource_mut::<CaMapMusic>().triggers.clear();
}

/// The client: music on / off from the server.
pub fn hear(
    mut on: MessageReader<CaMusicOn>,
    mut off: MessageReader<CaMusicOff>,
    mut music: ResMut<CaMusic>,
    clock: Res<super::MusicClock>,
) {
    for m in on.read() {
        music.start_direct(&normalize(&m.file), m.looping, m.fade as f64, clock.now);
    }
    if off.read().count() > 0 {
        music.player.stop();
    }
}
