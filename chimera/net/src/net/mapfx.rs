//! Map effects the server's logic sets off, for network clients to draw
//! (they run no map logic): strike-generator env_beams' strikes
//! (`map::BeamStrike`), point_tesla sparks
//! (`map::TeslaSpark`) and env_spark bursts
//! (`map::SparkBurst`). Unreliable, like Source's temporary
//! entities: a lost one is a missed flash.
//!
//! And map logic's sounds: its one-shots (`logic::LogicOneShot`, as
//! `ServerSound`) and its long-lived sounds (ambient_generic, movers'
//! loops, fire: `map::SoundControl` under the logic's keys, as
//! `LogicSound`, in order), played on each client as the server plays
//! them. A client that joins later doesn't hear loops already playing
//! (docs/tech-debt.md).
//!
//! Map effects for clients (they run no map logic): the server's
//! strike-generator beam strikes, tesla sparks and env_spark bursts as `MapFx`
//! (unreliable), written on a client as its map's own messages
//! (`map::BeamStrike`, `TeslaSpark`, `map::SparkBurst`, `map::MapShake`: env_shake's
//! and a canister's screen shakes reach network clients).
//! Players' and movers' render looks (`map::RenderLook`, `BrushTint`) are
//! replicated components; a client's `movers::copy_tints` puts a mover's on its
//! own node.

use bevy::prelude::*;
use bevy_replicon::prelude::*;

use super::{LogicSound, LogicSoundKind, MapFx, ServerSound};
use crate::{
    core::NetRole,
    map::{
        BeamStrike, LogicOneShot, MapBrushEntity, PlaySound, PropEntity, SoundControl, SoundKey, SparkBurst, StartSound,
        TeslaSpark,
    },
};

/// The logic's sound keys (`logic::bridge::sound_key`).
const LOGIC_KEYS: u64 = 1 << 63;

pub(super) fn plugin(app: &mut App) {
    app.add_message::<BeamStrike>()
        .add_message::<TeslaSpark>()
        .add_message::<SparkBurst>()
        .add_message::<LogicOneShot>()
        .add_message::<crate::map::logic_view::MapShake>()
        .add_message::<SoundControl>()
        .add_message::<PlaySound>()
        .add_systems(
            PostUpdate,
            (forward, forward_sounds)
                .before(ServerSystems::Send)
                .run_if(resource_equals(NetRole::Server)),
        )
        .add_systems(
            PreUpdate,
            (receive, receive_sounds)
                .after(ClientSystems::Receive)
                .run_if(resource_equals(NetRole::Client)),
        );
}

/// The server: this frame's map effects to every client.
fn forward(
    mut strikes: MessageReader<BeamStrike>,
    mut teslas: MessageReader<TeslaSpark>,
    mut sparks: MessageReader<SparkBurst>,
    mut shakes: MessageReader<crate::map::logic_view::MapShake>,
    mut out: MessageWriter<ToClients<MapFx>>,
) {
    let fx = strikes
        .read()
        .map(|s| MapFx::Strike {
            entity: s.entity as u32,
            start: s.start.to_array(),
            end: s.end.to_array(),
            life: s.life,
        })
        .chain(teslas.read().map(|t| MapFx::Tesla {
            entity: t.entity as u32,
        }))
        .chain(sparks.read().map(|s| MapFx::Spark {
            at: s.at.to_array(),
            dir: s.dir.to_array(),
            magnitude: s.magnitude,
        }))
        .chain(shakes.read().map(|s| MapFx::Shake {
            at: s.at.to_array(),
            amplitude: s.amplitude,
            frequency: s.frequency,
            duration: s.duration,
            radius: s.radius,
        }));
    for message in fx {
        out.write(ToClients {
            targets: SendTargets::CLIENTS_ONLY,
            message,
        });
    }
}

/// A client: the server's map effects as its own map's messages.
fn receive(mut fx: MessageReader<MapFx>, mut commands: Commands) {
    for f in fx.read().cloned() {
        commands.queue(move |w: &mut World| match f {
            MapFx::Strike {
                entity,
                start,
                end,
                life,
            } => {
                w.write_message(BeamStrike {
                    entity: entity as usize,
                    start: Vec3::from_array(start),
                    end: Vec3::from_array(end),
                    life,
                });
            }
            MapFx::Tesla { entity } => {
                w.write_message(TeslaSpark {
                    entity: entity as usize,
                });
            }
            MapFx::Spark { at, dir, magnitude } => {
                w.write_message(SparkBurst {
                    at: Vec3::from_array(at),
                    dir: Vec3::from_array(dir),
                    magnitude,
                });
            }
            MapFx::Shake {
                at,
                amplitude,
                frequency,
                duration,
                radius,
            } => {
                w.write_message(crate::map::logic_view::MapShake {
                    at: Vec3::from_array(at),
                    amplitude,
                    frequency,
                    duration,
                    radius,
                });
            }
        });
    }
}

/// The server: map logic's sounds to every client.
fn forward_sounds(
    mut shots: MessageReader<LogicOneShot>,
    mut controls: MessageReader<SoundControl>,
    replicated: Query<(), With<Replicated>>,
    parts: Query<(Option<&MapBrushEntity>, Option<&PropEntity>)>,
    mut sounds: MessageWriter<ToClients<ServerSound>>,
    mut out: MessageWriter<ToClients<LogicSound>>,
) {
    for s in shots.read() {
        let s = &s.0;
        sounds.write(ToClients {
            targets: SendTargets::CLIENTS_ONLY,
            message: ServerSound {
                entry: s.entry.clone(),
                at: s.at.map(|a| a.to_array()),
                volume: s.volume,
                pitch: s.pitch,
                source: None,
                channel: None,
            },
        });
    }
    let logic = |k: &SoundKey| k.0 & LOGIC_KEYS != 0;
    for c in controls.read() {
        let (key, what, follow) = match c {
            SoundControl::Start(s) if logic(&s.key) => {
                let follow = s.follow.filter(|e| replicated.contains(*e));
                let follow_part = s.follow.and_then(|e| match parts.get(e) {
                    Ok((Some(b), _)) => Some((true, b.0 as u32)),
                    Ok((_, Some(p))) => Some((false, p.0 as u32)),
                    _ => None,
                });
                let what = LogicSoundKind::Start {
                    entry: s.entry.clone(),
                    at: s.at.map(|a| a.to_array()),
                    follow_part,
                    volume: s.volume,
                    pitch: s.pitch,
                    level: s.level,
                    clip: s.clip.map(|c| c as u32),
                    looping: s.looping,
                    dry: s.dry,
                };
                (s.key.0, what, follow)
            }
            SoundControl::Change { key, volume, pitch } if logic(key) => (
                key.0,
                LogicSoundKind::Change {
                    volume: *volume,
                    pitch: *pitch,
                },
                None,
            ),
            SoundControl::Stop(key) if logic(key) => (key.0, LogicSoundKind::Stop, None),
            SoundControl::StopAll => (0, LogicSoundKind::StopAll, None),
            _ => continue,
        };
        out.write(ToClients {
            targets: SendTargets::CLIENTS_ONLY,
            message: LogicSound { key, what, follow },
        });
    }
}

/// A client: the server's logic sounds, played here.
fn receive_sounds(
    mut sounds: MessageReader<LogicSound>,
    parts: Query<(Entity, Option<&MapBrushEntity>, Option<&PropEntity>)>,
    mut out: MessageWriter<SoundControl>,
) {
    for s in sounds.read() {
        let key = SoundKey(s.key);
        let control = match &s.what {
            LogicSoundKind::Start {
                entry,
                at,
                follow_part,
                volume,
                pitch,
                level,
                clip,
                looping,
                dry,
            } => {
                // The client's own copy of the map part it follows.
                let part = follow_part.and_then(|(brush, i)| {
                    parts
                        .iter()
                        .find(|(_, b, p)| match brush {
                            true => b.is_some_and(|b| b.0 == i as usize),
                            false => p.is_some_and(|p| p.0 == i as usize),
                        })
                        .map(|(e, ..)| e)
                });
                SoundControl::Start(StartSound {
                    key,
                    entry: entry.clone(),
                    at: at.map(Vec3::from_array),
                    follow: s.follow.filter(|e| *e != Entity::PLACEHOLDER).or(part),
                    volume: *volume,
                    pitch: *pitch,
                    level: *level,
                    clip: clip.map(|c| c as usize),
                    looping: *looping,
                    dry: *dry,
                })
            }
            LogicSoundKind::Change { volume, pitch } => SoundControl::Change {
                key,
                volume: *volume,
                pitch: *pitch,
            },
            LogicSoundKind::Stop => SoundControl::Stop(key),
            LogicSoundKind::StopAll => SoundControl::StopAll,
        };
        out.write(control);
    }
}
