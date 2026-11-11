//! The server side: listening, the join handshake, a character per
//! client, intents from clients, and what characters replicate.
//!
//! The server: `listen` (`udp::UdpServer`: netcode, unsecure, `maxplayers` less
//! the host, queries on the same port), the join handshake (version, protocol,
//! `sv_password` from the client's netcode user data, full, the mount doctor's
//! report in its userinfo against the match manifest (`manifest`); refused with
//! `Refused` and dropped a second later), a character per client (team with
//! fewer players, out of the game (`core::Connecting`: dead, not solid, no
//! health) until its client reports the map loaded (`Loaded`, also after each
//! map change; the map kept on its client's entity: `ClientMap`), then
//! spawned as the rules allow (`rules::enter_game`), `Seed` from its id), each player's commands queued by tick (`CommandBuffer`: late
//! and too-early ones dropped; the lead reported for the client's newest clock
//! epoch only), `kick <name>` and `kickid <id> [message]` ("Kicked by Console")
//! and one applied per tick before the rules (the last repeated when missing),
//! replicon's tick kept equal to the simulation's (`lock_replication_tick`),
//! each client's updates paced by its `cl_updaterate` and `rate`
//! (`UpdatePacing`, `pace_messages`: its mutations dropped on ticks it isn't
//! due one; `RateLimits`: `sv_minupdaterate`, `sv_maxupdaterate`, `sv_minrate`,
//! `sv_maxrate`), its own state sent as a delta with its updates, after each
//! tick `NetBody` and each player's `OwnState` (predicted components, lead,
//! buffer) from the simulation's values, the host's and bots' characters marked
//! for replication, leaving players' characters and weapons removed; `players`
//! for `status`; a player who rejoins from the same address under the same name
//! while its old connection is silent drops that connection at once
//! (`PeerTable`, `drop_stale_twins`: the wire carries no account); players in the server log (`game_log::LogSubjects`: name,
//! client id, `STEAM_ID_LAN` or `BOT`).
//!
//! Known shortcuts (docs/tech-debt.md): Every character's body goes to
//! every client every update (no relevance culling): on de_dust2 only 37-45
//! % of character pairs are in each other's PVS (`heavy::net_capacity`
//! counts them), so Source-style culling would cut bodies (most of a
//! client's ~17 KB/s at 32 players) by about half. Not done: hiding a body
//! (replicon's per-client component visibility removes it on the client)
//! needs rules for what a client still gets of players it can't see
//! (footsteps and gunfire through walls, teammates on the radar: Source's
//! PAS and radar messages, to spec) and the client's handling of a body
//! that leaves and comes back (held, drawn at once, the soak's bodies-heard
//! check). Why it's OK for now: 32 players fit a home upload (~4.3 Mbit/s
//! for 31 remote clients at 20 updates a second; multiplayer.md
//! "Capacity"); a rented server has bandwidth to spare.

use std::{
    net::{SocketAddr, UdpSocket},
    time::{Duration, SystemTime},
};

use bevy::prelude::*;
use bevy_replicon::{prelude::*, server::server_tick::ServerTick, shared::backend::connected_client::NetworkId};
use bevy_replicon_renet::{
    RenetServer,
    netcode::{ServerAuthentication, ServerConfig},
};

use super::{
    HOST_ID, Join, NET_VERSION, NetBody, NetCharacter, NetCmd, NetEvent, NetSettings, NetVersion, OwnState,
    OwnStateDelta, PROTOCOL_ID, Refused, UserCmds, body_flags,
};
use crate::{
    character::character_bundle,
    core::{
        FreezeTime, Health, Intent, LocalPlayer, MovementState, NetRole, PredictedComponents, Seed, SimClock, SimSet,
        SimTick, Team, Velocity,
    },
    core::{Dead, Npc},
    slots::{Loadout, MovementSlot, set_movement},
    weapon::{
        Inventory,
    },
};

pub(super) fn plugin(app: &mut App) {
    use crate::console::ConsoleAppExt;
    app.console_command(
        "kick",
        "kick <name>: drop a player (or a bot) from the server.",
        |w, a| {
            let name = a.join(" ");
            if name.is_empty() {
                return Err("kick <name>".into());
            }
            kick_by(w, |n, _| n.eq_ignore_ascii_case(&name), "Kicked by Console")
                .ok_or_else(|| format!("No player named \"{name}\""))
                .map(Some)
        },
    )
    .console_command(
        "kickid",
        "kickid <userid> [message]: drop the player with that id (status lists them).",
        |w, a| {
            let id: u64 = a
                .first()
                .and_then(|i| i.parse().ok())
                .ok_or("kickid <userid> [message]")?;
            let message = a[1..].join(" ");
            let reason = if message.is_empty() {
                "Kicked by Console".to_string()
            } else {
                format!("Kicked by Console : {message}")
            };
            kick_by(w, |_, i| i == Some(id), &reason)
                .ok_or_else(|| format!("No player with userid {id}"))
                .map(Some)
        },
    );
    // Every game's, unless one names its players itself.
    chimera_core::gameplay::GameplayAppExt::any_game_hook(app, crate::game_log::LogSubjects(log_subject));
    app.add_observer(join)
        .add_observer(left)
        .init_resource::<PeerTable>()
        .add_systems(Update, drop_refused.run_if(in_state(ServerState::Running)))
        .add_systems(
            PreUpdate,
            track_peers
                .before(ServerSystems::Receive)
                .run_if(in_state(ServerState::Running)),
        )
        .add_systems(
            PreUpdate,
            (receive_commands, receive_loaded)
                .after(ServerSystems::Receive)
                .run_if(in_state(ServerState::Running)),
        )
        // Before the rules, which may hold the intent (the dead, freeze
        // time), as bots write theirs.
        .add_systems(
            FixedUpdate,
            apply_commands
                .before(SimSet::Rules)
                .run_if(resource_equals(NetRole::Server)),
        )
        // The tick's outcome, from the simulation's values (a listen
        // server draws eased transforms after the fixed loop).
        .add_systems(
            FixedLast,
            (canonical_positions, write_bodies, capture_own_states)
                .chain()
                .run_if(resource_equals(NetRole::Server)),
        )
        // Replicon's tick is the simulation's: a client keys snapshots by
        // the tick their message is from (`interp`).
        .add_systems(
            FixedPostUpdate,
            lock_replication_tick
                .after(ServerSystems::IncrementTick)
                .run_if(resource_equals(NetRole::Server)),
        )
        .add_systems(
            PostUpdate,
            (replicate_characters, pace_updates, send_own_states)
                .chain()
                .before(ServerSystems::Send)
                .run_if(resource_equals(NetRole::Server)),
        )
        // Clients not due an update this frame don't get its mutations.
        .add_systems(
            PostUpdate,
            pace_messages
                .after(ServerSystems::Send)
                .before(ServerSystems::SendPackets)
                .run_if(resource_equals(NetRole::Server)),
        )
        .init_resource::<RateLimits>();
    crate::console::resource_cvar::<RateLimits, f32>(
        app,
        "sv_minupdaterate",
        "Fewest updates a second the server sends a client, whatever its cl_updaterate.",
        |r| &mut r.min_updaterate,
    );
    crate::console::resource_cvar::<RateLimits, f32>(
        app,
        "sv_maxupdaterate",
        "Most updates a second the server sends a client (never more than its tick rate).",
        |r| &mut r.max_updaterate,
    );
    crate::console::resource_cvar::<RateLimits, f32>(
        app,
        "sv_minrate",
        "Least bandwidth (bytes a second) the server allows a client, whatever its rate.",
        |r| &mut r.min_rate,
    );
    crate::console::resource_cvar::<RateLimits, f32>(
        app,
        "sv_maxrate",
        "Most bandwidth (bytes a second) the server allows a client (0: no limit).",
        |r| &mut r.max_rate,
    );
}

/// The server's bounds on what clients ask for (Source's
/// `sv_minupdaterate`, `sv_maxupdaterate`, `sv_minrate`, `sv_maxrate`;
/// community-documented defaults, unmeasured: docs/tech-debt.md).
#[derive(Resource, Clone, Debug)]
pub struct RateLimits {
    pub min_updaterate: f32,
    pub max_updaterate: f32,
    pub min_rate: f32,
    /// 0: no limit.
    pub max_rate: f32,
}

impl Default for RateLimits {
    fn default() -> Self {
        Self {
            min_updaterate: 10.0,
            max_updaterate: 66.0,
            min_rate: 5000.0,
            max_rate: 0.0,
        }
    }
}

/// The userinfo keys of a client's update rate and bandwidth
/// (`predict::RateSettings`), and what a client that sent none gets.
pub const UPDATERATE_KEY: &str = "cl_updaterate";
pub const RATE_KEY: &str = "rate";
pub const DEFAULT_UPDATERATE: f32 = 20.0;
pub const DEFAULT_RATE: f32 = 30000.0;

/// On a client's server-side entity: when its next update is due and its
/// bandwidth left (Source's `cl_updaterate` and `rate`). A client is sent
/// the mutations of a tick (others' bodies, props, ...) and its own state
/// only on the ticks its update rate falls on, and none while it has
/// used more than its `rate` (a choke); reliable updates (spawns,
/// insertions, removals) and messages always go. What it misses is sent
/// with the next update (replicon sends what changed since the client's
/// last acknowledgement), and its interpolation draws across the gaps
/// (`cl_interp_ratio` update intervals).
#[derive(Component, Debug, Default, Clone)]
pub struct UpdatePacing {
    /// The tick (fractional) the next update is due at.
    next: f64,
    /// Bytes it may still be sent (negative: choked until refilled).
    budget: f64,
    /// The last tick paced, and the rate then (bytes a second).
    last_tick: u64,
    rate: f64,
    /// This frame's update goes to it.
    pub due: bool,
    /// Updates sent, and those a full budget held back.
    pub sent: u64,
    pub choked: u64,
}

/// How much bandwidth a client may save up, s of its rate; and owe after
/// a burst (a join's spawns, a map's download), s.
const RATE_BURST: f64 = 0.25;

/// Each client's update this frame: due when its update rate's interval
/// has passed since the last and its bandwidth allows.
fn pace_updates(
    clock: Res<SimClock>,
    limits: Res<RateLimits>,
    players: Query<&Player>,
    mut pacing: Query<(Entity, &mut UpdatePacing)>,
    info: Query<&crate::core::UserInfo>,
) {
    let tick = clock.tick;
    let dt = clock.delta.as_secs_f64().max(1e-4);
    for (client, mut pace) in &mut pacing {
        let ticks = tick.saturating_sub(pace.last_tick);
        if pace.last_tick == 0 {
            pace.next = tick as f64;
        }
        pace.last_tick = tick;
        // Replicon sends nothing in a frame without a tick.
        pace.due = false;
        if ticks == 0 {
            continue;
        }
        let user = players.get(client).ok().and_then(|p| info.get(p.character).ok());
        let get = |key: &str, default: f32| {
            user.and_then(|u| u.0.get(key))
                .and_then(|v| v.trim().parse::<f32>().ok())
                .filter(|v| v.is_finite())
                .unwrap_or(default)
        };
        let max_hz = limits.max_updaterate.max(1.0) as f64;
        let hz = (get(UPDATERATE_KEY, DEFAULT_UPDATERATE) as f64)
            .clamp(limits.min_updaterate.max(1.0) as f64, max_hz.max(limits.min_updaterate as f64));
        let interval = (1.0 / (hz * dt)).max(1.0);
        let mut rate = (get(RATE_KEY, DEFAULT_RATE) as f64).max(limits.min_rate as f64);
        if limits.max_rate > 0.0 {
            rate = rate.min(limits.max_rate as f64);
        }
        pace.rate = rate;
        pace.budget = (pace.budget + rate * dt * ticks as f64).min(rate * RATE_BURST);
        if (tick as f64) + 1e-6 < pace.next {
            continue;
        }
        if pace.budget < 0.0 {
            pace.choked += 1;
            continue;
        }
        pace.due = true;
        pace.sent += 1;
        pace.next += interval;
        if pace.next <= tick as f64 {
            pace.next = tick as f64 + interval;
        }
    }
}

/// After replicon has built this frame's messages: mutations only to the
/// clients due an update; what each is sent counts against its rate.
pub fn pace_messages(
    mut messages: ResMut<bevy_replicon::shared::backend::server_messages::ServerMessages>,
    mut pacing: Query<&mut UpdatePacing>,
) {
    let mutations: usize = bevy_replicon::shared::backend::channels::ServerChannel::Mutations.into();
    messages.retain_sent(|(client, channel, _)| *channel != mutations || pacing.get(*client).is_ok_and(|p| p.due));
    for (client, _, bytes) in messages.iter_sent() {
        if let Ok(mut p) = pacing.get_mut(client) {
            p.budget -= bytes.len() as f64;
        }
    }
    for mut p in &mut pacing {
        let floor = -p.rate * RATE_BURST;
        if p.budget < floor {
            p.budget = floor;
        }
    }
}

/// On a client's server-side entity (replicon's `ConnectedClient`): its
/// player's name and character.
#[derive(Component, Debug)]
pub struct Player {
    pub name: String,
    pub character: Entity,
}

/// On a refused client's entity: when it was told (`drop_refused`).
#[derive(Component, Debug)]
struct RefusedAt(f64);

/// How long a refused client has to hear why before it is dropped, s.
const REFUSED_GRACE: f64 = 1.0;

/// Kick the first player (or bot: no id) whose name and id `pick` takes:
/// a client is told why and dropped a moment later (as a refusal); a bot
/// leaves at once and the quota goes down by one. What happened, or None
/// when nobody matched (or no game is served).
fn kick_by(world: &mut World, pick: impl Fn(&str, Option<u64>) -> bool, reason: &str) -> Option<String> {
    if world.get_resource::<NetRole>() != Some(&NetRole::Server) {
        return None;
    }
    let client = world
        .query::<(Entity, &Player, &NetworkId)>()
        .iter(world)
        .find(|(_, p, id)| pick(&p.name, Some(id.get())))
        .map(|(c, p, _)| (c, p.name.clone()));
    if let Some((client, name)) = client {
        info!("kicked {name}: {reason}");
        world.commands().server_trigger(ToClients {
            targets: SendTargets::Single(ClientId::Client(client)),
            message: Refused {
                reason: reason.to_string(),
            },
        });
        let at = world.resource::<Time<Real>>().elapsed_secs_f64();
        world
            .entity_mut(client)
            .insert((RefusedAt(at), LeaveReason(reason.to_string())));
        return Some(format!("kicked {name}"));
    }
    let bot = world
        .query_filtered::<(Entity, &Name), With<crate::core::Bot>>()
        .iter(world)
        .find(|(_, n)| pick(n.as_str(), None))
        .map(|(e, n)| (e, n.as_str().to_string()));
    let (bot, name) = bot?;
    // The game's (`BotHooks`): CS:S drops the bomb it carries and lowers
    // `bot_quota`.
    let hooks = super::BotHooks::active(world);
    (hooks.kick)(world, bot);
    Some(format!("kicked {name}"))
}

/// Kick the person with user id `id` (`kickid`'s path: told `reason`,
/// dropped a moment later); a listen server's host can't be kicked.
/// What happened, or None when nobody has that id.
pub fn kick_userid(world: &mut World, id: u64, reason: &str) -> Option<String> {
    if id == HOST_ID {
        return None;
    }
    kick_by(world, |_, i| i == Some(id), reason)
}

/// Each connected client's address and silence, by client id, kept
/// current from the UDP transport (empty on the memory link; a test sets
/// it). A client has no account on the wire, so a returning player is
/// told by address and name (`drop_stale_twins`).
#[derive(Resource, Default, Debug)]
pub struct PeerTable(pub std::collections::BTreeMap<u64, (std::net::IpAddr, Duration)>);

/// A connection this long silent is dead, not a second client of the same
/// name on the same machine.
const STALE_AFTER: Duration = Duration::from_secs(2);

fn track_peers(udp: Option<Res<super::udp::UdpServer>>, mut table: ResMut<PeerTable>) {
    if let Some(udp) = udp {
        table.0 = udp.peers();
    }
}

/// A player who dropped (a killed client) and joins again shows twice
/// until the dead connection times out. A player named `name` from the
/// same address as new client `client` (id `id`), silent for
/// `STALE_AFTER`, is that connection: dropped at once. Two live clients
/// of one name on one machine stay (neither is silent).
fn drop_stale_twins(world: &mut World, client: Entity, id: u64, name: &str) {
    let Some(&(ip, _)) = world.resource::<PeerTable>().0.get(&id) else { return };
    let stale: Vec<(Entity, u64)> = world
        .query::<(Entity, &Player, &NetworkId)>()
        .iter(world)
        .filter(|(e, p, _)| *e != client && p.name == name)
        .map(|(e, _, n)| (e, n.get()))
        .filter(|(_, other)| {
            world
                .resource::<PeerTable>()
                .0
                .get(other)
                .is_some_and(|(addr, silent)| *addr == ip && *silent >= STALE_AFTER)
        })
        .collect();
    for (old, other) in stale {
        info!("{name}: dropping client {other}, silent, for the rejoin (client {id})");
        world.entity_mut(old).insert(LeaveReason("Reconnected.".to_string()));
        world.write_message(DisconnectRequest { client: old });
    }
}

/// Drop refused clients that didn't hang up.
fn drop_refused(q: Query<(Entity, &RefusedAt)>, time: Res<Time<Real>>, mut out: MessageWriter<DisconnectRequest>) {
    let now = time.elapsed_secs_f64();
    for (client, at) in &q {
        if now - at.0 >= REFUSED_GRACE {
            out.write(DisconnectRequest { client });
        }
    }
}

/// On a remote player's character: the commands its client sent for
/// ticks to come (a jitter buffer, plan §1 "bind command number to server
/// tick"), the last one applied, and how they arrive.
#[derive(Component, Debug, Default)]
pub struct CommandBuffer {
    pub queued: std::collections::BTreeMap<u64, NetCmd>,
    /// The command run last (repeated for a tick without one).
    pub last: Option<NetCmd>,
    /// The last tick a command was applied for.
    pub applied: u64,
    /// The smallest lead heard since the last `OwnState` (see
    /// `OwnState::lead`), and the newest tick heard.
    pub lead: Option<i32>,
    pub newest: u64,
    /// The newest clock epoch heard (`UserCmds::epoch`): `lead` is that
    /// epoch's.
    pub epoch: u32,
    /// Ticks run on a repeated command; commands that came too late (after
    /// their tick) or too early (beyond `MAX_AHEAD`).
    pub missed: u32,
    pub late: u32,
    pub early: u32,
    /// The newest own state its client holds (`UserCmds::ack`, from the
    /// last message heard).
    pub ack: u64,
}

/// Commands further ahead of the server than this (ticks) are dropped: a
/// client can't queue up more than this, whatever its clock says.
pub const MAX_AHEAD: u64 = 64;

/// On a remote player's character: its state after the latest tick, for
/// its client (`send_own_states`).
#[derive(Component, Debug, Default)]
pub struct OwnStateOut {
    state: Option<OwnState>,
    sent: u64,
    /// The states sent lately (tick, encoded), oldest first: the bases
    /// of the deltas (`OwnStateDelta`).
    history: std::collections::VecDeque<(u64, Vec<u8>)>,
    /// The last one's size on the wire and whole, bytes.
    pub last_sizes: (usize, usize),
}

/// Own states kept as delta bases (2 s at 64 Hz: more than a round trip).
const OWN_HISTORY: usize = 128;

impl OwnStateOut {
    /// The newest state captured (sent or to be sent).
    pub fn state(&self) -> Option<&OwnState> {
        self.state.as_ref()
    }
}

/// Start serving the loaded map on `ip`:`hostport` (a listen server: this
/// process's own player plays too). Fails if a network game is running
/// or the port is taken.
pub fn listen(world: &mut World) -> Result<SocketAddr, String> {
    let NetSettings { ip, hostport: port, .. } = *world.resource::<NetSettings>();
    let socket = UdpSocket::bind((ip, port)).map_err(|e| format!("UDP {ip} port {port}: {e}"))?;
    let addr = socket.local_addr().map_err(|e| e.to_string())?;
    let max_clients = remote_slots(world);
    let config = ServerConfig {
        current_time: SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default(),
        max_clients,
        protocol_id: PROTOCOL_ID,
        // Direct IP and LAN, as Source's game traffic: no encryption.
        // Secure connect tokens come with a matchmaker (plan slice 9).
        public_addresses: vec![addr],
        authentication: ServerAuthentication::Unsecure,
    };
    // Answers server queries on the same port (`query`).
    let secret = world.resource::<super::query::InstanceId>().0 ^ config.current_time.as_nanos() as u64;
    let transport = super::udp::UdpServer::new(config, socket, secret).map_err(|e| e.to_string())?;
    start(world)?;
    world.insert_resource(transport);
    world.write_message(NetEvent::Listening(addr));
    Ok(addr)
}

/// Become the server with renet's `RenetServer` (a transport is added by
/// the caller: netcode in `listen`, memory in tests).
pub fn start(world: &mut World) -> Result<(), String> {
    if super::active(world) {
        return Err("already in a network game (disconnect first)".into());
    }
    let config = super::connection_config(world);
    world.insert_resource(RenetServer::new(config));
    world.insert_resource(NetRole::Server);
    // Bots added before serving stay (`bot_quota` counts them).
    let hooks = super::BotHooks::active(world);
    (hooks.sync_quota)(world);
    Ok(())
}

/// Remote players the server takes: `maxplayers`, less the host's own
/// slot on a listen server (one with a local player).
fn remote_slots(world: &mut World) -> usize {
    let max = world.resource::<NetSettings>().maxplayers.max(1) as usize;
    let host = world.query_filtered::<(), With<LocalPlayer>>().iter(world).count();
    max.saturating_sub(host).max(1)
}

/// Stop serving: clients are told and dropped, their characters go.
pub(super) fn stop(world: &mut World) {
    if let Some(mut transport) = world.remove_resource::<super::udp::UdpServer>() {
        if let Some(mut server) = world.get_resource_mut::<RenetServer>() {
            transport.disconnect_all(&mut server);
        }
    }
    if let Some(memory) = world.remove_resource::<super::memory::MemoryServer>()
        && let Some(server) = world.get_resource::<RenetServer>()
    {
        memory.hang_up(server);
    }
    world.remove_resource::<RenetServer>();
    let clients: Vec<Entity> = world
        .query_filtered::<Entity, With<ConnectedClient>>()
        .iter(world)
        .collect();
    for c in clients {
        despawn_player(world, c);
        world.despawn(c);
    }
    // What stays (the host, bots) stops replicating.
    let replicated: Vec<Entity> = world.query_filtered::<Entity, With<Replicated>>().iter(world).collect();
    for e in replicated {
        world
            .entity_mut(e)
            .remove::<(
                Replicated,
                NetCharacter,
                NetBody,
                super::NetProp,
                super::NetRagdoll,
                super::NetHeld,
                super::NetItem,
                super::NetSmoke,
                super::NetScore,
            )>();
    }
    // The parts' own (`NetHooks::stopped_serving`: CS:S's round entity,
    // bomb and hostages; Source's movers).
    let hooks = world.resource::<super::NetHooks>().stopped_serving.clone();
    for stopped in hooks {
        stopped(world);
    }
}

/// A connected client says who it is: refused (another version or
/// protocol, the server full) or let in with a character.
fn join(join: On<FromClient<Join>>, mut commands: Commands) {
    let Some(client) = join.client_id.entity() else {
        return;
    };
    let msg = join.message.clone();
    commands.queue(move |world: &mut World| admit(world, client, msg));
}

fn admit(world: &mut World, client: Entity, msg: Join) {
    let ours = world.resource::<NetVersion>().0.clone();
    let protocol = *world.resource::<ProtocolHash>();
    let refuse = if msg.version != ours {
        Some(format!(
            "Your game is version {} but the server's is {ours}: update the one that's older.",
            msg.version
        ))
    } else if msg.protocol != protocol {
        Some(format!(
            "Your game's network protocol differs from the server's ({NET_VERSION})."
        ))
    } else if world.get::<Player>(client).is_some() {
        return;
    } else if !password_ok(world, client) {
        // Source's words.
        Some("Bad password.".to_string())
    } else {
        let taken = world.query_filtered::<(), With<Player>>().iter(world).count();
        if taken >= remote_slots(world) {
            Some("Server is full.".to_string())
        } else {
            // The mount doctor's report (none: taken as having all).
            let report = super::manifest::from_userinfo(msg.userinfo.iter().map(|(k, v)| (k.as_str(), v.as_str())));
            report.and_then(|mut r| {
                let manifest = super::manifest::current(world);
                // A client that fetches the offered games' files has them.
                super::assets::count_offered(&manifest, &msg.userinfo, &mut r);
                let games = world.get_resource::<chimera_core::game_registry::Games>();
                super::manifest::check(&manifest, &r, games)
                    .err()
                    .map(|why| why + &super::assets::refusal_hint(&manifest).unwrap_or_default())
            })
        }
    };
    let id = world.get::<NetworkId>(client).map_or(0, |n| n.get());
    if let Some(reason) = refuse {
        info!("refused client {id} ({}): {reason}", msg.name);
        world.commands().server_trigger(ToClients {
            targets: SendTargets::Single(ClientId::Client(client)),
            message: Refused { reason },
        });
        // Dropped a moment later: the client hangs up when it hears why,
        // and a disconnect sent now could overtake the reason.
        let at = world.resource::<Time<Real>>().elapsed_secs_f64();
        world.entity_mut(client).insert(RefusedAt(at));
        return;
    }
    let name = clean_name(&msg.name, id);
    drop_stale_twins(world, client, id, &name);
    // How it comes in (`JoinRules`; CS:S's `rules::JoinChoice`, in its
    // userinfo): on a team at once unless it asked for the join menus.
    let choice = msg
        .userinfo
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(JOIN_KEY))
        .map(|(_, v)| v.clone());
    let character = spawn_player(world, id, &name, choice.as_deref());
    set_userinfo(world, character, &msg.userinfo);
    world.entity_mut(client).insert((
        AuthorizedClient,
        Player {
            name: name.clone(),
            character,
        },
        UpdatePacing::default(),
    ));
    // The served map (its offer: name, hash, size, where to download it).
    let welcome = super::maps::welcome(world, id);
    world.commands().server_trigger(ToClients {
        targets: SendTargets::Single(ClientId::Client(client)),
        message: welcome,
    });
    // The server's replicated cvars (movement, rules), before it plays.
    super::cvars::send_all(world, client);
    // The message of the day, for its join menus.
    super::motd::send(world, client);
    info!("{name} joined (client {id})");
    // Everyone's chat: "Player X has joined the game".
    world.write_message(ToClients {
        targets: SendTargets::All,
        message: super::PlayerCame { name, left: None },
    });
}

/// Whether a client gave the server's password (`sv_password`; any
/// without one). It rides in netcode's user data (`query::user_data`);
/// a client without a UDP transport (tests' memory link) gives none.
fn password_ok(world: &World, client: Entity) -> bool {
    let want = &world.resource::<super::query::Hosting>().password;
    if want.is_empty() {
        return true;
    }
    let id = world.get::<NetworkId>(client).map(|n| n.get());
    let data = id.and_then(|id| world.get_resource::<super::udp::UdpServer>()?.user_data(id));
    super::query::password_of(data.as_ref()) == *want
}

/// A remote player's userinfo (`core::UserInfo`), bounded: at most 64
/// keys of 64 bytes, values of 256.
pub(super) fn set_userinfo(world: &mut World, character: Entity, values: &[(String, String)]) {
    let info = crate::core::UserInfo(
        values
            .iter()
            .filter(|(k, v)| k.len() <= 64 && v.len() <= 256)
            .take(64)
            .map(|(k, v)| (k.to_lowercase(), v.clone()))
            .collect(),
    );
    if let Ok(mut e) = world.get_entity_mut(character) {
        e.insert((super::chat::ClientUserInfo(info.0.clone()), info));
    }
}

/// A printable name of at most 32 characters, or "Player <id>".
pub(super) fn clean_name(name: &str, id: u64) -> String {
    let n: String = name.chars().filter(|c| !c.is_control()).take(32).collect();
    let n = n.trim();
    if n.is_empty() {
        format!("Player {id}")
    } else {
        n.to_string()
    }
}

/// A remote player's character: on the team with fewer players, out of
/// the game (`core::Connecting`) until its client has loaded the map,
/// then spawned as the rules allow (`JoinRules`; CS:S's
/// `rules::enter_game`).
fn spawn_player(world: &mut World, id: u64, name: &str, choice: Option<&str>) -> Entity {
    let mut counts = [0usize; 2];
    for (team, npc) in world.query::<(&Team, Has<Npc>)>().iter(world) {
        if !npc && (1..=2).contains(&team.0) {
            counts[team.0 as usize - 1] += 1;
        }
    }
    // Counter-terrorists (2) on a tie, as the local player starts.
    let team = if counts[0] < counts[1] { Team(1) } else { Team(2) };
    // The match's movement, never noclip (CS:S spawns every player
    // walking; specs/cs_source/movement.md "Noclip and spawning").
    let movement = world
        .get_resource::<Loadout>()
        .map(|l| l.movement)
        .filter(|m| *m != crate::movement::noclip::ID)
        .unwrap_or(crate::movement::placeholder::ID);
    let e = world
        .spawn((
            character_bundle(Transform::default(), team),
            // Command numbers and shared randoms differ per player.
            Seed(0x5EED_0000_0000_0000 ^ id),
            Dead { since: f64::MIN },
            CommandBuffer::default(),
            OwnStateOut::default(),
            crate::weapon::lagcomp::ViewTick::default(),
            crate::core::RemotePlayer,
            Replicated,
            NetCharacter {
                owner: Some(id),
                name: name.to_string(),
            },
            NetBody::default(),
        ))
        .id();
    world.entity_mut(e).insert(Name::new(name.to_string()));
    set_movement(e, movement).apply(world);
    // Out of the game until its client has the map (`receive_loaded`), on
    // the team (or menus) it asked for: the game's join flow.
    let rules = super::JoinRules::active(world);
    (rules.joined)(world, e, choice);
    e
}

/// The userinfo key a client says how it joins with (`JoinRules::choice`;
/// CS:S's `rules::JoinChoice::key`).
pub const JOIN_KEY: &str = "_jointeam";

/// On a client's entity: the map its client last reported loaded
/// (`Loaded`) while the server was on it (a match waits for it after a
/// map change: `minigame_net`).
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub struct ClientMap(pub String);

/// A client has the map loaded: its player comes into the game, if it's
/// the map the server is on (a report from before a map change counts
/// for nothing).
fn receive_loaded(mut loaded: MessageReader<FromClient<super::Loaded>>, players: Query<&Player>, mut commands: Commands) {
    for l in loaded.read() {
        let Some(client) = l.client_id.entity() else { continue };
        let Ok(p) = players.get(client) else { continue };
        let (character, map) = (p.character, l.message.map.clone());
        commands.queue(move |world: &mut World| {
            let serving = world
                .get_resource::<super::maps::ServedMap>()
                .map_or_else(|| super::current_map(world), |s| s.map.clone());
            if !map.eq_ignore_ascii_case(&serving) || world.get_entity(character).is_err() {
                info!("a client reported {map} loaded; the server is on {serving}");
                return;
            }
            if let Ok(mut c) = world.get_entity_mut(client) {
                c.insert(ClientMap(serving));
            }
            let rules = super::JoinRules::active(world);
            (rules.enter)(world, character);
            // What was shot up before it came.
            super::decals::send(world, client);
        });
    }
}

/// Why a client is being dropped (a kick's reason), for the others' chat.
#[derive(Component, Clone, Debug)]
pub struct LeaveReason(pub String);

/// A client left (or was dropped): its character and weapons go, and
/// everyone reads why ("Player X left the game (Disconnect by user.)"):
/// a kick's reason, netcode's timeout ("timed out"), else the player
/// hung up.
fn left(
    remove: On<Remove<ConnectedClient>>,
    players: Query<(&Player, Option<&NetworkId>, Option<&LeaveReason>)>,
    mut commands: Commands,
) {
    let Ok((p, id, reason)) = players.get(remove.entity) else { return };
    let (name, character) = (p.name.clone(), p.character);
    let (id, reason) = (id.map(|n| n.get()), reason.map(|r| r.0.clone()));
    commands.queue(move |world: &mut World| {
        let transport = id.and_then(|id| world.get_resource_mut::<super::udp::UdpServer>()?.left.remove(&id));
        let reason = reason
            .or(transport.map(str::to_string))
            .unwrap_or_else(|| super::client::BY_USER.to_string());
        if super::chat::hosted(world) {
            debug!("{name} left ({reason})");
        } else {
            info!("{name} left ({reason})");
        }
        despawn_character(world, character);
        world.write_message(ToClients {
            targets: SendTargets::All,
            message: super::PlayerCame { name, left: Some(reason) },
        });
    });
}

fn despawn_player(world: &mut World, client: Entity) {
    if let Some(character) = world.get::<Player>(client).map(|p| p.character) {
        despawn_character(world, character);
    }
}

/// A character and the weapons it carries.
fn despawn_character(world: &mut World, character: Entity) {
    let weapons = world
        .get::<Inventory>(character)
        .map(|i| i.weapons.clone())
        .unwrap_or_default();
    for w in weapons {
        if let Ok(e) = world.get_entity_mut(w) {
            e.despawn();
        }
    }
    if let Ok(e) = world.get_entity_mut(character) {
        e.despawn();
    }
}

/// Queue each remote player's commands by tick, and note how early they
/// came (the clock sync's lead). Commands for ticks already run are late
/// (counted, dropped); ones more than `MAX_AHEAD` ahead are dropped. A
/// command already queued for a tick is replaced (the client moved its
/// clock back and re-predicted that tick); repeats from `CMD_BACKUP` are
/// the same command.
fn receive_commands(
    mut messages: MessageReader<FromClient<UserCmds>>,
    players: Query<&Player>,
    mut buffers: Query<&mut CommandBuffer>,
    tick: Res<SimTick>,
) {
    let now = tick.0;
    for msg in messages.read() {
        let Some(client) = msg.client_id.entity() else {
            continue;
        };
        let Ok(p) = players.get(client) else { continue };
        let Ok(mut b) = buffers.get_mut(p.character) else {
            continue;
        };
        // The state to send deltas against, also from a message without
        // commands (a client not predicting yet, or missing a base).
        b.ack = msg.message.ack;
        let Some(newest) = msg.message.cmds.iter().map(|c| c.tick).max() else {
            continue;
        };
        let lead = (newest as i64 - now as i64).clamp(i32::MIN as i64, i32::MAX as i64) as i32;
        // Leads of the client's present clock only: commands sent before
        // it jumped say nothing of where it is now.
        if msg.message.epoch > b.epoch {
            b.epoch = msg.message.epoch;
            b.lead = None;
        }
        if msg.message.epoch == b.epoch {
            b.lead = Some(b.lead.map_or(lead, |l| l.min(lead)));
        }
        b.newest = b.newest.max(newest);
        if newest <= b.applied {
            // Even its newest command's tick has run.
            b.late += 1;
        }
        for cmd in &msg.message.cmds {
            if cmd.tick <= b.applied {
                continue;
            }
            if cmd.tick > now + MAX_AHEAD {
                b.early += 1;
                continue;
            }
            b.queued.insert(cmd.tick, cmd.clone());
        }
    }
}

/// Each remote player's command for this tick into its `Intent`, made
/// safe (`NetCmd::apply`): exactly one command per tick whatever the
/// client sends, so no client moves faster than the tick allows. Without
/// one (lost, late) the last repeats, and the client gets corrected.
fn apply_commands(
    mut q: Query<(
        &mut CommandBuffer,
        &mut Intent,
        Option<&mut crate::weapon::lagcomp::ViewTick>,
        Option<&crate::core::ViewSet>,
    )>,
    clock: Res<SimClock>,
) {
    let tick = clock.tick;
    for (mut b, mut intent, view, turned) in &mut q {
        // Commands for ticks gone by never run.
        let stale: Vec<u64> = b.queued.range(..tick).map(|(t, _)| *t).collect();
        for t in stale {
            b.queued.remove(&t);
        }
        let cmd = match b.queued.remove(&tick) {
            Some(c) => Some(c),
            None => {
                if b.last.is_some() {
                    b.missed += 1;
                }
                b.last.clone()
            }
        };
        b.applied = tick;
        if let Some(c) = cmd {
            let look = (intent.yaw, intent.pitch);
            c.apply(&mut intent);
            // A look the server turned (a spawn facing) stands until the
            // client has taken it.
            if turned.is_some_and(|v| v.count != c.look_ack) {
                (intent.yaw, intent.pitch) = look;
            }
            // Where it saw others: its shots are traced there.
            if let Some(mut view) = view {
                let v = crate::weapon::lagcomp::ViewTick(c.view(tick));
                if *view != v {
                    *view = v;
                }
            }
            b.last = Some(c);
        }
    }
}

/// Remote players' positions with their zeros as +0.0 (`canonical_position`).
fn canonical_positions(world: &mut World) {
    let players: Vec<Entity> = world
        .query_filtered::<Entity, With<OwnStateOut>>()
        .iter(world)
        .collect();
    for e in players {
        super::canonical_position(world, e);
    }
}

/// After each tick: every remote player's predicted state for its client.
fn capture_own_states(world: &mut World) {
    let clock = *world.resource::<SimClock>();
    let tick_nanos = world.resource::<Time<Fixed>>().timestep().as_nanos() as u64;
    let time_nanos = world.resource::<Time<Fixed>>().elapsed().as_nanos() as u64;
    let frozen = world.get_resource::<FreezeTime>().is_some_and(|f| f.0);
    // The freeze ends on the first tick at its end time (`Holds`; CS:S's
    // `rules::rounds`).
    let holds = super::Holds::active(world);
    let frozen_until = match (holds.freeze_ends)(world) {
        Some(until) if frozen => {
            let dt = clock.delta.as_secs_f64().max(1e-6);
            let k = ((until - clock.now) / dt - 1e-6).ceil().max(1.0);
            Some(clock.tick + k as u64)
        }
        _ if frozen => Some(u64::MAX),
        _ => None,
    };
    let characters: Vec<Entity> = world
        .query_filtered::<Entity, With<OwnStateOut>>()
        .iter(world)
        .collect();
    let own_parts = world.remove_resource::<super::parts::OwnParts>();
    world.resource_scope(|world, registry: Mut<PredictedComponents>| {
        for e in characters {
            let state = registry.encode(world, e);
            let movement = world.get::<MovementSlot>(e).map_or("", |m| m.0).to_string();
            let seed = world.get::<Seed>(e).map_or(0, |s| s.0);
            let held = world.get::<Dead>(e).is_some();
            let alive = !held && world.get::<Health>(e).is_none_or(|h| h.current > 0.0);
            let rebased = world.get::<crate::core::Rebased>(e).map_or(0, |r| r.0);
            let parts = own_parts.as_ref().map_or_else(Vec::new, |p| p.encode(world, e));
            let view = world
                .get::<crate::core::MapView>(e)
                .map(|v| (v.origin.to_array(), v.rotation.to_array()));
            let ground = world
                .get::<MovementState>(e)
                .and_then(|s| s.ground)
                .and_then(|g| world.get::<crate::map::MapBrushEntity>(g))
                .map(|m| m.0 as u32);
            let look = world.get::<crate::core::ViewSet>(e).map(|v| (v.count, v.yaw, v.pitch));
            let mut out = world.get_mut::<OwnStateOut>(e).expect("queried");
            out.state = Some(OwnState {
                tick: clock.tick,
                time_nanos,
                tick_nanos,
                state,
                movement,
                seed,
                held,
                alive,
                ground,
                frozen_until,
                parts,
                view,
                look,
                rebased,
                ..default()
            });
        }
    });
    if let Some(p) = own_parts {
        world.insert_resource(p);
    }
}

/// The newest state of each remote player to its client, with how its
/// commands are arriving: with its updates only (`UpdatePacing`), as a
/// delta against the newest state it holds (`OwnStateDelta`).
fn send_own_states(
    players: Query<(Entity, &Player)>,
    mut characters: Query<(&mut OwnStateOut, &mut CommandBuffer)>,
    pacing: Query<&UpdatePacing>,
    mut out: MessageWriter<ToClients<OwnStateDelta>>,
) {
    for (client, p) in &players {
        let Ok((mut own, mut b)) = characters.get_mut(p.character) else {
            continue;
        };
        // Only with its updates (`UpdatePacing`).
        if pacing.get(client).is_ok_and(|p| !p.due) {
            continue;
        }
        let Some(mut state) = own.state.clone() else { continue };
        if state.tick <= own.sent {
            continue;
        }
        own.sent = state.tick;
        state.lead = b.lead.take();
        state.newest = b.newest;
        state.epoch = b.epoch;
        state.buffered = b.queued.len().min(u16::MAX as usize) as u16;
        state.missed = b.missed;
        state.late = b.late;
        // A delta against the newest state the client holds, if we still
        // have it; else against nothing (joining, a loss of every state
        // in our history, a client that started over).
        let Ok(bytes) = postcard::to_allocvec(&state) else { continue };
        let base = own.history.iter().rev().find(|(t, _)| *t == b.ack && b.ack != 0);
        let (base_tick, delta) = match base {
            Some((t, base)) => (*t, super::delta::encode(base, &bytes)),
            None => (0, super::delta::encode(&[], &bytes)),
        };
        own.last_sizes = (delta.len(), bytes.len());
        own.history.push_back((state.tick, bytes));
        while own.history.len() > OWN_HISTORY {
            own.history.pop_front();
        }
        out.write(ToClients {
            targets: SendTargets::Single(ClientId::Client(client)),
            message: OwnStateDelta {
                tick: state.tick,
                base: base_tick,
                delta,
            },
        });
    }
}
/// Replicon's tick (the tick replication messages carry) kept equal to
/// the simulation's (`SimClock::tick`), so a value a client receives is
/// keyed by the server tick it is from. Both count once per fixed tick;
/// this only moves replicon's forward to it once, when serving starts.
fn lock_replication_tick(clock: Res<SimClock>, mut tick: ResMut<ServerTick>) {
    let want = clock.tick as u32;
    let diff = want.wrapping_sub(tick.get());
    if diff != 0 && (diff as i32) > 0 {
        tick.increment_by(diff);
    }
}

/// Characters this server didn't spawn for a client (the host's, bots)
/// replicate too.
#[allow(clippy::type_complexity)]
fn replicate_characters(
    q: Query<
        (Entity, Option<&Name>, Has<LocalPlayer>),
        (With<Intent>, With<Health>, Without<Replicated>, Without<Npc>),
    >,
    settings: Res<NetSettings>,
    mut commands: Commands,
) {
    for (e, name, local) in &q {
        let name = if local {
            settings.name.clone()
        } else {
            name.map_or_else(|| "Character".into(), |n| n.as_str().to_string())
        };
        if local {
            // The host's own character is called what the others see
            // (the HUD's kill feed and leader line read `Name`).
            commands.entity(e).insert(Name::new(name.clone()));
        }
        commands.entity(e).insert((
            Replicated,
            NetCharacter {
                owner: local.then_some(HOST_ID),
                name,
            },
            NetBody::default(),
        ));
    }
}

/// The simulation's state of each character into what replicates.
fn write_bodies(
    clock: Res<SimClock>,
    mut q: Query<(&Transform, &Velocity, &MovementState, &Intent, Has<Dead>, &mut NetBody)>,
) {
    let far = crate::map::interp::SNAP_SPEED * clock.delta.as_secs_f32();
    for (t, v, s, i, dead, mut body) in &mut q {
        // A teleport counts one up (`body_flags::TELEPORTS`).
        let mut flags = body.flags & body_flags::TELEPORTS;
        if Vec3::from_array(body.origin).distance(t.translation) > far {
            flags = (flags + body_flags::TELEPORT_ONE) & body_flags::TELEPORTS;
        }
        for (on, bit) in [
            (s.on_ground, body_flags::ON_GROUND),
            (s.crouching, body_flags::CROUCHING),
            (s.on_ladder, body_flags::ON_LADDER),
            (dead, body_flags::DEAD),
        ] {
            if on {
                flags |= bit;
            }
        }
        let now = NetBody {
            origin: t.translation.to_array(),
            velocity: v.0.to_array(),
            yaw: i.yaw,
            pitch: i.pitch,
            eye: s.eye_offset.to_array(),
            flags,
        }
        .quantized();
        body.set_if_neq(now);
    }
}

/// A connected player, for `status` and the scoreboard.
#[derive(Clone, Debug)]
pub struct PlayerInfo {
    pub id: u64,
    pub name: String,
    /// Round trip, ms (None for the host).
    pub ping_ms: Option<f64>,
    pub loss: f64,
    /// Commands waiting for later ticks, and ticks run without one
    /// (`CommandBuffer`); 0 for the host.
    pub buffered: usize,
    pub missed: u32,
}

/// The host (on a listen server) and every joined client.
pub fn players(world: &mut World) -> Vec<PlayerInfo> {
    let mut out = Vec::new();
    let host = world.resource::<NetSettings>().name.clone();
    if world
        .query_filtered::<(), With<LocalPlayer>>()
        .iter(world)
        .next()
        .is_some()
    {
        out.push(PlayerInfo {
            id: HOST_ID,
            name: host,
            ping_ms: None,
            loss: 0.0,
            buffered: 0,
            missed: 0,
        });
    }
    let mut clients = Vec::new();
    for (p, id, stats) in world
        .query::<(&Player, &NetworkId, Option<&ConnectedClientStats>)>()
        .iter(world)
    {
        clients.push((
            p.character,
            PlayerInfo {
                id: id.get(),
                name: p.name.clone(),
                ping_ms: stats.map(|s| s.rtt * 1000.0),
                loss: stats.map_or(0.0, |s| s.packet_loss),
                buffered: 0,
                missed: 0,
            },
        ));
    }
    for (character, mut info) in clients {
        if let Some(b) = world.get::<CommandBuffer>(character) {
            info.buffered = b.queued.len();
            info.missed = b.missed;
        }
        out.push(info);
    }
    out
}

/// A player in the server log (`game_log`): its name and client id as the
/// userid (as `users` lists them; the host's is 0), `BOT` for bots (their
/// entity index as the userid). Characters the network doesn't know: the
/// default.
fn log_subject(world: &World, e: Entity) -> Option<crate::game_log::LogSubject> {
    let Some(c) = world.get::<NetCharacter>(e) else {
        return crate::game_log::default_subject(world, e);
    };
    Some(crate::game_log::LogSubject {
        name: c.name.clone(),
        userid: c.owner.unwrap_or(e.index().index() as u64),
        unique: if c.owner.is_some() { "STEAM_ID_LAN" } else { "BOT" }.into(),
    })
}
