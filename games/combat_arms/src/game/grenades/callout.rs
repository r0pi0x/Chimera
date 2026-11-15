//! The throw call-out (specs/combat_arms/grenades.md, Corrections
//! (2026-10-27) C1; effects_sounds.md 11): the voice that calls a grenade
//! out, not a weapon sound.
//!
//! When the server makes a grenade from a fire message (`grenades::launch`;
//! not a death's release) and the weapon calls out (`GrenadeParams::
//! call_out`: `PlayAutoThrowSound` on and `Guntype` 7), it sends the
//! thrower's team `CaThrowCallout` (the auto-cast record
//! `GrenadeThrowing`: outer radius 500, files `EventSounds::grenade_*` by
//! the thrower's voice, one of two at random). `send` gives it to the
//! readers on the thrower's team, the thrower included (`chat::readers`,
//! as the music triggers do: this machine's player directly, a remote
//! player as a server message); other teams get it only as others' voice
//! lines (`party_hears`: a reader who asked for them with
//! `party_voice_lines_3d`, `map::sound::PartyVoices`, within 2R of the
//! thrower; the party apps' default). `hear` plays it
//! on a client: 3D at the thrower (with the thrower as its source), volume
//! 100, the default volume class, and dropped while the client's previous
//! call-out still plays (`CalloutLimiter`).
//!
//! Known shortcuts: the sound is placed at the thrower when it starts and
//! does not follow them as they move (Chimera's one-shots fix their gain
//! at the start: `effects::sounds`); the voice is the body's skeleton's
//! (0 male, 1 female), the male VIP's template is not told apart.

use bevy::{ecs::entity::MapEntities, prelude::*};
use serde::{Deserialize, Serialize};

use super::super::{
    characters::records::Selection,
    chat,
    data::CaData,
    effects::{
        CaSoundLengths,
        data::CaFxTables,
        sounds::{CHANNEL_CALLOUT, CaSound},
        voice::VOICE_DEFAULT_LENGTH,
    },
};
use crate::{core::Team, weapon::random::shared_random};

/// The skeleton of the female bodies (`weapons::FEMALE`).
const FEMALE_SKELETON: usize = 1;

/// Server to the thrower's team: a grenade was thrown.
#[derive(Message, Serialize, Deserialize, Clone, Debug, PartialEq, MapEntities)]
pub struct CaThrowCallout {
    #[entities]
    pub thrower: Entity,
    /// The voice file inside the install's archives.
    pub file: String,
    /// Where the thrower was (metres).
    pub at: [f32; 3],
    /// The outer radius (u).
    pub radius: f32,
}

/// The files of the call-out for a voice (`female`), from the tables.
pub fn files(tables: &CaFxTables, female: bool) -> &[String] {
    let s = &tables.sounds;
    if female {
        &s.grenade_throw_female
    } else {
        &s.grenade_throw
    }
}

/// Whether `thrower`'s body is a female one (its skeleton).
pub fn is_female(world: &World, thrower: Entity) -> bool {
    let (Some(ca), Some(look)) = (world.get_resource::<CaData>(), world.get::<Selection>(thrower)) else {
        return false;
    };
    ca.characters().effective(look).0.skeleton == FEMALE_SKELETON
}

/// The server: the call-out of `thrower`'s throw, to its team. `command`
/// seeds the file pick.
pub fn send(world: &mut World, thrower: Entity, command: u32) {
    let (Some(team), Some(at)) = (
        world.get::<Team>(thrower).copied(),
        world.get::<Transform>(thrower).map(|t| t.translation),
    ) else {
        return;
    };
    let Some(tables) = world.get_resource::<CaFxTables>() else {
        return;
    };
    let list = files(tables, is_female(world, thrower));
    if list.is_empty() {
        return;
    }
    let pick = (shared_random(command, "combat_arms:callout", 0).next() * list.len() as f32) as usize;
    let message = CaThrowCallout {
        thrower,
        file: list[pick.min(list.len() - 1)].clone(),
        at: at.to_array(),
        radius: tables.sounds.grenade_throw_radius,
    };
    for (target, reader) in chat::readers(world) {
        if world.get::<Team>(reader).is_some_and(|t| *t == team) || party_hears(world, reader, thrower, at, message.radius)
        {
            chat::send(world, target, message.clone());
        }
    }
}

/// Whether `reader`, beyond the speaker's team, gets `speaker`'s voice
/// line of outer radius `radius` (u) spoken at `at`: it asked for others'
/// voice lines (`map::sound::PartyVoices`, its userinfo) and is within the
/// server's send reach of a sound that big (`effects::rules::server_sends`,
/// 2R).
pub fn party_hears(world: &World, reader: Entity, speaker: Entity, at: Vec3, radius: f32) -> bool {
    use super::super::{effects::rules::server_sends, inventory::UNIT};
    reader != speaker
        && crate::map::sound::PartyVoices::wants(world, reader)
        && world
            .get::<Transform>(reader)
            .is_some_and(|t| server_sends(t.translation.distance(at) / UNIT, radius))
}

/// A client's grenade call-out while it plays (until when): one at a time.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CalloutLimiter(pub f64);

impl CalloutLimiter {
    /// Start a call-out lasting `length` s at `now` unless one still plays.
    pub fn try_start(&mut self, now: f64, length: f64) -> bool {
        if now < self.0 {
            return false;
        }
        self.0 = now + length;
        true
    }
}

/// The client: play the call-outs that arrived.
pub fn hear(
    mut calls: MessageReader<CaThrowCallout>,
    characters: Query<&Transform>,
    lengths: Res<CaSoundLengths>,
    time: Res<Time<Real>>,
    mut limiter: Local<CalloutLimiter>,
    mut sounds: MessageWriter<CaSound>,
) {
    let now = time.elapsed_secs_f64();
    for c in calls.read() {
        let length = lengths.0.get(&c.file).copied().unwrap_or(VOICE_DEFAULT_LENGTH) as f64;
        if !limiter.try_start(now, length) {
            continue;
        }
        let at = characters.get(c.thrower).map_or(Vec3::from(c.at), |t| t.translation);
        sounds.write(CaSound::at(&c.file, at, c.radius).from_source(c.thrower, CHANNEL_CALLOUT));
    }
}
