//! The rules every game has, in a network game
//! (docs/plans/active/multiplayer.md, slice 5): everything here is the
//! server's, and a client shows it. A game's own (CS:S's round, bomb and
//! hostages: `cs_source::game::net`) are its part (`NetParts`).
//!
//! - **Scores** (`NetScore` on every character): kills, deaths, ping, a
//!   bot, what it carries that marks it (`core::ScoreMark`: CS:S's bomb)
//!   and a defusal kit, for the scoreboard. A client's own money, armour
//!   and kit come in its `OwnState` parts (`parts::OwnParts`): money is shown only to its owner, as
//!   in CS:S.
//! - **Buying** (`BuyRequest`) is the server's to check (buy zone, buy
//!   time, money, team-only items: `weapon::economy::buy`); a refusal
//!   comes back as `Notice`, the game's hint (`notify`, which a game's
//!   part uses for its own requests).
//! - **The rules' sounds** as `ServerSound` (`map::GameSound`: CS:S's
//!   beeps, plant, defuse, blast, hostages), a player's own predicted ones
//!   (`map::PredictedSound`) to everyone but that player's client.
//! - **Time**: the server's times go as server ticks (`tick_of`), which a
//!   client puts on its own clock (`ServerTime`).
//!
//! The rules over the network (slice 5): scoreboard lines (`NetScore`: kills,
//! deaths, ping, bot/marks/kit), buy requests (`economy::buy`; refusals as
//! `Notice`), the rules' sounds; a client writes them into single player's
//! components (`Score`, `DefuseKit`, its own `Money`, `Armor`, `NightVision`)
//! with times moved onto its clock (`ServerTime`).

use bevy::{ecs::message::MessageCursor, prelude::*};
use bevy_replicon::prelude::*;

use super::{
    BuyRequest, NetCharacter, NetScore, Notice, OwnState, ServerSound, interp::InterpClock, parts::OwnParts,
    predict::CommandClock,
    score_flags, server::Player,
};
use crate::{
    core::{LocalPlayer, NetRole, Npc, Score, ScoreMark, SimClock},
    map::{GameSound, PlaySound, PredictedSound},
    weapon::{
        Inventory,
        economy::{BuyRequested, DefuseKit},
    },
};

pub(super) fn plugin(app: &mut App) {
    let server = || resource_equals(NetRole::Server);
    let client = || resource_equals(NetRole::Client);
    app.add_systems(FixedLast, write_scores.run_if(server()))
        .add_systems(
            PreUpdate,
            receive_buys
                .after(ServerSystems::Receive)
                .run_if(in_state(ServerState::Running)),
        )
        .add_systems(PostUpdate, send_sounds.before(ServerSystems::Send).run_if(server()))
        .add_systems(
            PreUpdate,
            (apply_scores, apply_own, receive_sounds)
                .chain()
                .after(ClientSystems::Receive)
                .after(super::predict::rebuild_own_states)
                .run_if(client()),
        )
        .add_systems(PostUpdate, send_buys.before(ClientSystems::Send).run_if(client()));
}

// --- Time

/// A server time (`SimClock::now`, s) as a server tick (fractional), by
/// the tick `clock` is at.
pub fn tick_of(clock: &SimClock, t: f64) -> f64 {
    clock.tick as f64 + (t - clock.now) / clock.delta.as_secs_f64().max(1e-9)
}

/// A client's view of the server's clock: the server tick it is at now
/// (as its updates arrive: the interpolation clock's render tick plus
/// the delay), the tick length, and this client's fixed clock now. A
/// server tick `t` is `fixed + (t - tick) × step` on the client's clock.
#[derive(Clone, Copy, Debug)]
pub struct ServerTime {
    pub tick: f64,
    pub step: f64,
    pub fixed: f64,
}

impl ServerTime {
    pub fn of(world: &World) -> Option<Self> {
        let interp = world.get_resource::<InterpClock>()?;
        if !interp.running {
            return None;
        }
        let fixed = world.resource::<Time<Fixed>>();
        let step = world
            .get_resource::<CommandClock>()
            .and_then(|c| c.step())
            .unwrap_or(fixed.timestep())
            .as_secs_f64()
            .max(1e-6);
        Some(Self {
            tick: interp.render_tick + interp.delay / step,
            step,
            fixed: fixed.elapsed_secs_f64(),
        })
    }

    /// Server tick `t` on this client's fixed clock (s).
    pub fn local(&self, t: f64) -> f64 {
        self.fixed + (t - self.tick) * self.step
    }
}

// --- Server

/// A connection losing this share of its packets or more is marked
/// (`score_flags::LOSS`).
pub const LOSS_MARK: f64 = 0.01;

/// Each character's scoreboard line. Ping (and packet loss, marked from
/// `LOSS_MARK`) is measured again every `PING_TICKS` ticks (Source's
/// scoreboard updates about once a second).
#[allow(clippy::type_complexity)]
fn write_scores(
    mut q: Query<
        (
            Entity,
            &NetCharacter,
            Option<&Score>,
            Option<&Inventory>,
            Has<DefuseKit>,
            Option<&mut NetScore>,
        ),
        Without<Npc>,
    >,
    players: Query<(&Player, Option<&ConnectedClientStats>)>,
    marks: Query<&ScoreMark>,
    clock: Res<SimClock>,
    mut commands: Commands,
) {
    const PING_TICKS: u64 = 64;
    let measure = clock.tick % PING_TICKS == 0;
    for (e, c, score, inv, kit, have) in &mut q {
        let s = score.copied().unwrap_or_default();
        let mut flags = 0;
        if c.owner.is_none() {
            flags |= score_flags::BOT;
        }
        // What it carries marks it (CS:S's bomb: `score_flags::BOMB`).
        for w in inv.iter().flat_map(|i| i.weapons.iter()) {
            if let Ok(m) = marks.get(*w) {
                flags |= m.0;
            }
        }
        if kit {
            flags |= score_flags::KIT;
        }
        let (ping, loss) = if measure || have.is_none() {
            players
                .iter()
                .find(|(p, _)| p.character == e)
                .and_then(|(_, stats)| stats)
                .map_or((0, false), |s| ((s.rtt * 1000.0).round().clamp(0.0, u16::MAX as f64) as u16, s.packet_loss >= LOSS_MARK))
        } else {
            have.as_ref().map_or((0, false), |h| (h.ping, h.flags & score_flags::LOSS != 0))
        };
        if loss {
            flags |= score_flags::LOSS;
        }
        let now = NetScore {
            kills: s.kills,
            deaths: s.deaths,
            ping,
            flags,
        };
        match have {
            Some(mut h) => {
                h.set_if_neq(now);
            }
            None => {
                commands.entity(e).insert(now);
            }
        }
    }
}

/// A client asked to buy something: the rules decide (`economy::buy`);
/// why not goes back as the game's hint.
fn receive_buys(mut requests: MessageReader<FromClient<BuyRequest>>, players: Query<&Player>, mut commands: Commands) {
    for r in requests.read() {
        let Some(client) = r.client_id.entity() else { continue };
        let Ok(p) = players.get(client) else { continue };
        let (character, what) = (p.character, r.message.what.chars().take(64).collect::<String>());
        commands.queue(move |w: &mut World| {
            if w.get_entity(character).is_err() {
                return;
            }
            if let Err(text) = crate::weapon::economy::buy(w, character, &what) {
                notify(w, client, text);
            }
        });
    }
}

/// The game's hint to one client.
pub fn notify(world: &mut World, client: Entity, text: String) {
    world.write_message(ToClients {
        targets: SendTargets::Single(ClientId::Client(client)),
        message: Notice { text },
    });
}

/// The rules' sounds, to every client; a sound of a player's predicted
/// action (`PredictedSound`) to every client but that player's, which
/// played it already.
fn send_sounds(
    mut sounds: MessageReader<GameSound>,
    mut predicted: MessageReader<PredictedSound>,
    replicated: Query<(), With<Replicated>>,
    players: Query<(Entity, &Player)>,
    mut out: MessageWriter<ToClients<ServerSound>>,
) {
    let all = sounds.read().map(|s| (&s.0, None));
    let own = predicted.read().map(|s| (&s.sound, Some(s.by)));
    for (s, by) in all.chain(own) {
        let client = by.and_then(|by| players.iter().find(|(_, p)| p.character == by).map(|(c, _)| c));
        out.write(ToClients {
            targets: client.map_or(SendTargets::CLIENTS_ONLY, |c| SendTargets::AllExcept(ClientId::Client(c))),
            message: ServerSound {
                entry: s.entry.clone(),
                at: s.at.map(|a| a.to_array()),
                volume: s.volume,
                pitch: s.pitch,
                source: s.source.filter(|e| replicated.contains(*e)),
                channel: s.channel,
            },
        });
    }
}

// --- Client

/// Scores and kits into `Score` and `DefuseKit` (the scoreboard and HUD
/// read them as in single player).
fn apply_scores(q: Query<(Entity, Ref<NetScore>, Option<&Score>, Has<DefuseKit>)>, mut commands: Commands) {
    for (e, s, score, kit) in &q {
        if !s.is_changed() && score.is_some() {
            continue;
        }
        if score.is_none_or(|x| x.kills != s.kills || x.deaths != s.deaths) {
            commands.entity(e).insert(Score {
                kills: s.kills,
                deaths: s.deaths,
            });
        }
        let has = s.flags & score_flags::KIT != 0;
        if has && !kit {
            commands.entity(e).insert(DefuseKit);
        } else if !has && kit {
            commands.entity(e).remove::<DefuseKit>();
        }
    }
}

/// Our own-state parts (money, armour and the like: `parts::OwnParts`)
/// from the server's newest state (arming and defusing are predicted:
/// `objectives::bomb::arm`).
fn apply_own(world: &mut World, mut cursor: Local<MessageCursor<OwnState>>) {
    let newest = cursor
        .read(world.resource::<Messages<OwnState>>())
        .max_by_key(|s| s.tick)
        .map(|s| s.parts.clone());
    let Some(blob) = newest else { return };
    let Some(player) = chimera_core::query_cache::first::<With<LocalPlayer>>(world) else {
        return;
    };
    world.resource_scope(|world, parts: Mut<OwnParts>| parts.apply(world, player, &blob));
}

/// The rules' sounds the server played.
fn receive_sounds(mut sounds: MessageReader<ServerSound>, mut play: MessageWriter<PlaySound>) {
    for s in sounds.read() {
        play.write(PlaySound {
            entry: s.entry.clone(),
            at: s.at.map(Vec3::from_array),
            volume: s.volume,
            pitch: s.pitch,
            source: s.source.filter(|e| *e != Entity::PLACEHOLDER),
            channel: s.channel,
        });
    }
}

/// Our buys, to the server.
fn send_buys(mut asked: MessageReader<BuyRequested>, mut out: MessageWriter<BuyRequest>) {
    for a in asked.read() {
        out.write(BuyRequest { what: a.0.clone() });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_times_become_ticks() {
        let clock = SimClock {
            tick: 100,
            now: 1.5,
            delta: std::time::Duration::from_secs_f64(0.015),
        };
        assert!((tick_of(&clock, 1.5 + 0.15) - 110.0).abs() < 1e-9);
        let t = ServerTime {
            tick: 105.0,
            step: 0.015,
            fixed: 20.0,
        };
        assert!((t.local(110.0) - 20.075).abs() < 1e-9);
    }
}
