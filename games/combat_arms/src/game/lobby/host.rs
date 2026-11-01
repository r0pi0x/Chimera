//! The lobby on the engine: the lobby service hosted by a game process (a
//! listen game, a dedicated server, or a game of one), its messages over
//! Chimera's network, the client's copy, and the launcher that turns a
//! room's start into the process's own match (specs/combat_arms/
//! lobby.md F, S, G, X, M).
//!
//! - `CaLobbyHost` (the server's resource, off by default: `ca_lobby 1`)
//!   holds the `Lobby`. A client's `CaLobbyRequest` (a game of one: the
//!   local player's, as player 0) goes into it; the notices it pushes
//!   go back as `CaLobbyNotice`s to each recipient (`deliver`).
//! - The launcher stands in for the service's game server: a room's start
//!   (`LobbyEvent::Launch`) applies its settings to the match (mode, goal,
//!   friendly fire, join in progress, the Kill Cam option, restart) and,
//!   after `launch_delay` seconds, reports the game server ready. One
//!   process runs one match: a second room's start while one is running
//!   is not served and times out as the spec's launch does.
//! - Players are in the lobby until their room's "go": while the lobby is
//!   on, a player's character is held out of the match (`core::
//!   Connecting`, `holds`) unless it carries `CaInMatch`. "Go" sends it in
//!   on its room team with the kit the lobby built (`OwnKit`); the
//!   match's end (`Phase::MatchOver`) closes the room, and after
//!   `result_secs` the room is waiting again and every player is held out
//!   once more.
//! - `CaLobbyClient` is the client's copy (`client::ClientLobby`): notices
//!   in, the screens' requests out. It says hello when the local player
//!   exists.

use std::collections::HashMap;

use bevy::prelude::*;
use bevy_replicon::prelude::*;
use serde::{Deserialize, Serialize};

use super::{
    client::ClientLobby,
    data::LobbyData,
    room::*,
    service::{Lobby, LobbyConfig},
};
use crate::{
    console::resource_cvar,
    core::{Connecting, LocalPlayer, NetRole, SimClock, Team as MatchTeam},
    net::{NetAppExt, server::Player},
    weapon::{OwnKit, WeaponRegistry},
};
use crate::game::{
    chat::{self, Target},
    death_cam::CaKillCamRoom,
    rules::{CaMatch, CaRuleSettings, Phase, flow},
};

/// A request to the lobby.
#[derive(Message, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct CaLobbyRequest(pub Request);

/// A notice from the lobby.
#[derive(Message, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct CaLobbyNotice(pub Notice);

/// A character that went into the match from its room.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct CaInMatch;

/// The match a room started on this process.
#[derive(Clone, Debug, PartialEq)]
pub struct Active {
    pub room: RoomId,
    /// The game server reports ready at this time (`SimClock`).
    pub ready_at: f64,
    /// The players went in.
    pub went: bool,
    /// The room is waiting again at this time, once the match is over.
    pub over_at: Option<f64>,
}

/// The lobby hosted by this process.
#[derive(Resource)]
pub struct CaLobbyHost {
    pub lobby: Lobby,
    /// The lobby runs (`ca_lobby 1`).
    pub enabled: bool,
    /// The lobby tick, s.
    pub tick_secs: f64,
    next_tick: f64,
    /// The launcher's time to a ready game server, s.
    pub launch_delay: f64,
    /// The match's end to the room waiting again, s.
    pub result_secs: f64,
    /// Load the room's map when it starts (the apps do; headless tests
    /// don't have the install).
    pub load_maps: bool,
    /// Players whose connection is gone leave the lobby (off in tests that
    /// make up players).
    pub drop_leavers: bool,
    pub active: Option<Active>,
    /// Every notice that was delivered, for the log of tests.
    pub delivered: usize,
}

impl Default for CaLobbyHost {
    fn default() -> Self {
        Self {
            lobby: Lobby::new(
                LobbyData::built_in(),
                super::items::Catalog::built_in(),
                LobbyConfig::default(),
            ),
            enabled: false,
            tick_secs: 1.0,
            next_tick: 0.0,
            launch_delay: 0.0,
            result_secs: 10.0,
            load_maps: false,
            drop_leavers: true,
            active: None,
            delivered: 0,
        }
    }
}

/// The client's copy of the lobby.
#[derive(Resource)]
pub struct CaLobbyClient(pub ClientLobby);

/// The local player's name in the lobby (`ca_lobby_name`).
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct CaLobbyName(pub String);

impl Default for CaLobbyName {
    fn default() -> Self {
        Self("Player".into())
    }
}

/// The lobby's systems.
#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
pub struct CaLobbySet;

pub struct CaLobbyPlugin;

impl Plugin for CaLobbyPlugin {
    fn build(&self, app: &mut App) {
        if !app.world().contains_resource::<CaLobbyHost>() {
            let catalog = super::items::Catalog::over(super::super::data::CaData::of(app).gear());
            app.insert_resource(CaLobbyHost {
                lobby: Lobby::new(LobbyData::built_in(), catalog, LobbyConfig::default()),
                ..default()
            });
        }
        app.init_resource::<CaLobbyName>()
            .insert_resource(CaLobbyClient(ClientLobby::new(LobbyData::built_in(), 0, "Player")))
            .add_message::<CaLobbyRequest>()
            .add_message::<CaLobbyNotice>()
            .add_systems(
                Update,
                (
                    client_frame,
                    serve_standalone.run_if(standalone),
                    host_frame,
                )
                    .chain()
                    .in_set(CaLobbySet),
            );
        resource_cvar::<CaLobbyName, String>(app, "combat_arms:ca_lobby_name", "Your name in the lobby.", |n| {
            &mut n.0
        });
        crate::console::ConsoleAppExt::console_command(
            app,
            "combat_arms:ca_lobby",
            "ca_lobby [0|1]: run the lobby on this process (rooms, ready, start); players wait in it until their \
             room starts.",
            |w, a| {
                let mut host = w.resource_mut::<CaLobbyHost>();
                if let Some(v) = a.first() {
                    host.enabled = v.parse::<f32>().map_err(|_| "ca_lobby [0|1]")? != 0.0;
                    if host.enabled {
                        host.load_maps = true;
                    }
                }
                Ok(Some(format!("ca_lobby {}", host.enabled as u8)))
            },
        );
        crate::console::ConsoleAppExt::console_command(
            app,
            "combat_arms:ca_lobby_demo",
            "ca_lobby_demo [n]: a demo room (Elimination) for the local player with n made-up players (screenshots, \
             checking the screens); they stay until the lobby is restarted.",
            |w, a| {
                let n: u64 = a.first().and_then(|v| v.parse().ok()).unwrap_or(5);
                demo_room(w, n.min(15))?;
                Ok(Some(format!("a demo room with {n} made-up players")))
            },
        );
        app.net_part("combat_arms_lobby", register);
    }
}

/// `ca_lobby_demo`: the local player hosts a room with made-up players
/// (ids from 1000: they have no connection, so they are kept).
fn demo_room(world: &mut World, n: u64) -> Result<(), String> {
    let name = world.resource::<CaLobbyName>().0.clone();
    let mut host = world.resource_mut::<CaLobbyHost>();
    if !host.enabled {
        return Err("the lobby is off: ca_lobby 1".into());
    }
    host.drop_leavers = false;
    let lobby = &mut host.lobby;
    let data = lobby.data.clone();
    let m = data.mode_by_key("TDM").ok_or("no Elimination")?;
    lobby.request(0, Request::Hello { name }).map_err(|e| e.text())?;
    lobby
        .request(
            0,
            Request::Create(CreateSpec {
                name: "Demo Room".into(),
                password: String::new(),
                mode: m.id,
                pure_combat: false,
                map: None,
                goal: m.goal_default(),
                players: 16,
                arms: [0, 0],
            }),
        )
        .map_err(|e| e.text())?;
    let room = lobby.room_of(0).ok_or("no room")?;
    const NAMES: [&str; 8] = ["Ghost", "Viper", "Anvil", "Rook", "Sable", "Drift", "Quill", "Hex"];
    for i in 0..n {
        let id = 1000 + i;
        let who = format!("{}{}", NAMES[i as usize % NAMES.len()], i / 8 + 1);
        lobby.request(id, Request::Hello { name: who }).map_err(|e| e.text())?;
        lobby.request(id, Request::Enter { room, password: String::new() }).map_err(|e| e.text())?;
        if i % 2 == 0 {
            let _ = lobby.request(id, Request::Ready(true));
        }
    }
    Ok(())
}

fn standalone(role: Option<Res<NetRole>>) -> bool {
    role.is_none_or(|r| *r == NetRole::Standalone)
}

/// The protocol (in this order on every side) and the server's receiving.
fn register(app: &mut App) {
    app.add_client_message::<CaLobbyRequest>(Channel::Ordered)
        .add_server_message::<CaLobbyNotice>(Channel::Ordered)
        .add_systems(
            PreUpdate,
            receive
                .after(ServerSystems::Receive)
                .run_if(in_state(ServerState::Running)),
        );
}

/// The lobby id of a transport client: its entity's bits; the process's
/// own local player is 0.
pub fn id_of(client: ClientId) -> PlayerId {
    client.entity().map_or(0, |e| e.to_bits())
}

fn target_of(world: &World, p: PlayerId) -> Target {
    if p == 0 {
        if world.get_resource::<NetRole>().is_none_or(|r| *r == NetRole::Standalone) {
            Target::Local
        } else {
            Target::Client(ClientId::Server)
        }
    } else {
        Target::Client(ClientId::Client(Entity::from_bits(p)))
    }
}

/// A network server: the clients' requests (the host's too).
fn receive(mut requests: MessageReader<FromClient<CaLobbyRequest>>, mut commands: Commands) {
    for r in requests.read() {
        let (from, req) = (id_of(r.client_id), r.message.0.clone());
        commands.queue(move |w: &mut World| {
            let Some(mut host) = w.get_resource_mut::<CaLobbyHost>() else { return };
            if host.enabled {
                let _ = host.lobby.request(from, req);
            }
        });
    }
}

/// A game of one: its player's requests, served as a server would.
fn serve_standalone(world: &mut World) {
    let requests: Vec<CaLobbyRequest> = world.resource_mut::<Messages<CaLobbyRequest>>().drain().collect();
    let mut host = world.resource_mut::<CaLobbyHost>();
    if !host.enabled {
        return;
    }
    for r in requests {
        let _ = host.lobby.request(0, r.0);
    }
}

/// The client's side: notices in, its screens' requests out, hello.
fn client_frame(world: &mut World) {
    let now = chat::now_ms(world);
    let notices: Vec<CaLobbyNotice> = world.resource_mut::<Messages<CaLobbyNotice>>().drain().collect();
    let has_player = chimera_core::query_cache::first::<With<LocalPlayer>>(world).is_some();
    let name = world.resource::<CaLobbyName>().0.clone();
    let mut out = Vec::new();
    {
        let mut c = world.resource_mut::<CaLobbyClient>();
        for n in notices {
            c.0.apply(n.0, now);
        }
        c.0.tick(now);
        if has_player && !c.0.hello_sent {
            c.0.hello_sent = true;
            c.0.name = name.clone();
            c.0.send(Request::Hello { name });
        }
        out.extend(c.0.take_outbox());
    }
    for r in out {
        world.write_message(CaLobbyRequest(r));
    }
}

/// The host's frame: leavers, the lobby tick, the launcher, the match's
/// end, the players held out, the notices delivered.
fn host_frame(world: &mut World) {
    if !world.resource::<CaLobbyHost>().enabled {
        return;
    }
    if world.get_resource::<NetRole>() == Some(&NetRole::Client) {
        return;
    }
    let now = world.resource::<SimClock>().now;
    if world.resource::<CaLobbyHost>().drop_leavers {
        drop_leavers(world);
    }
    world.resource_scope(|world, mut host: Mut<CaLobbyHost>| {
        if now >= host.next_tick {
            host.next_tick = now + host.tick_secs;
            host.lobby.tick();
        }
        // The launcher reports the game server ready.
        if let Some(a) = &host.active
            && !a.went
            && now >= a.ready_at
        {
            let room = a.room;
            host.lobby.server_ready(room);
        }
        for e in host.lobby.take_events() {
            handle(world, &mut host, e, now);
        }
        end_of_match(world, &mut host, now);
    });
    hold_outsiders(world);
    deliver(world);
}

/// Players whose connection is gone leave the lobby.
fn drop_leavers(world: &mut World) {
    let present: Vec<PlayerId> = world.query::<(Entity, &Player)>().iter(world).map(|(e, _)| e.to_bits()).collect();
    let local = chimera_core::query_cache::first::<With<LocalPlayer>>(world).is_some();
    let mut host = world.resource_mut::<CaLobbyHost>();
    let gone: Vec<PlayerId> = host
        .lobby
        .players()
        .into_iter()
        .filter(|p| if *p == 0 { !local } else { !present.contains(p) })
        .collect();
    for p in gone {
        host.lobby.disconnect(p);
    }
}

/// Send every notice to its recipients.
pub fn deliver(world: &mut World) {
    let (notices, recipients): (Vec<(Audience, Notice)>, Vec<Vec<PlayerId>>) = {
        let mut host = world.resource_mut::<CaLobbyHost>();
        let notices = host.lobby.take_notices();
        host.delivered += notices.len();
        let recipients = notices.iter().map(|(a, _)| host.lobby.recipients(*a)).collect();
        (notices, recipients)
    };
    for ((_, n), to) in notices.into_iter().zip(recipients) {
        for p in to {
            let t = target_of(world, p);
            chat::send(world, t, CaLobbyNotice(n.clone()));
        }
    }
}

// ---------------------------------------------------------------------------
// The launcher

fn handle(world: &mut World, host: &mut CaLobbyHost, e: LobbyEvent, now: f64) {
    match e {
        LobbyEvent::Launch { room, settings, .. } => {
            if host.active.is_some() {
                // One match per process: this room's launch is not served.
                return;
            }
            apply_launch(world, host, &settings);
            host.active = Some(Active {
                room,
                ready_at: now + host.launch_delay,
                went: false,
                over_at: None,
            });
        }
        LobbyEvent::StopLauncher(room) => {
            if host.active.as_ref().is_some_and(|a| a.room == room && !a.went) {
                host.active = None;
            }
        }
        LobbyEvent::Expect { room, player, team, weapons } => {
            if let Some(a) = &mut host.active
                && a.room == room
            {
                a.went = true;
            }
            enter_player(world, host, player, team, &weapons);
        }
        LobbyEvent::Dropped { player, .. } => hold_player(world, player),
    }
}

/// A room's settings become the match's: the mode, the goal, friendly
/// fire, join in progress, the Kill Cam option, the map (when the host
/// loads maps) and a restart.
fn apply_launch(world: &mut World, host: &CaLobbyHost, s: &Settings) {
    let data = host.lobby.data.clone();
    let Some(mode) = data.mode(s.mode) else { return };
    {
        let mut rules = world.resource_mut::<CaRuleSettings>();
        rules.mode = mode.key.clone();
        rules.goal = s.goal;
        rules.friendly_fire = s.friendly_fire as u8;
        rules.join_in_progress = s.join_mid as u8;
    }
    world.insert_resource(CaKillCamRoom(s.kill_cam as i32));
    if host.load_maps
        && let Some(map) = data.map(s.map)
    {
        let line = format!("changelevel {}{}", crate::game::world::MAP_PREFIX, map.key);
        world.resource_mut::<crate::console::Console>().submit(line);
    }
    flow::restart(world);
}

/// The character of a lobby player.
fn character_of(world: &mut World, p: PlayerId) -> Option<Entity> {
    if p == 0 {
        chimera_core::query_cache::first::<With<LocalPlayer>>(world)
    } else {
        let client = Entity::from_bits(p);
        world.get::<Player>(client).map(|pl| pl.character)
    }
}

/// "Go": the player's character goes into the match on its room team with
/// the kit the lobby built.
fn enter_player(world: &mut World, host: &CaLobbyHost, p: PlayerId, team: Team, weapons: &[i32]) {
    let Some(e) = character_of(world, p) else { return };
    let ids: Vec<&'static str> = {
        let registry = world.resource::<WeaponRegistry>();
        weapons
            .iter()
            .filter_map(|n| host.lobby.catalog.get(*n))
            .filter_map(|i| i.weapon.as_deref())
            .filter_map(|w| registry.find(w).map(|d| d.id))
            .collect()
    };
    crate::game::inventory::respawn_inventory(world, e);
    let mut ent = world.entity_mut(e);
    ent.insert((CaInMatch, MatchTeam(team.match_team())));
    if !ids.is_empty() {
        ent.insert(OwnKit(ids));
    }
    flow::enter_game(world, e);
}

/// A player is out of the match (left the room, or the room is waiting
/// again): held out until their next "go".
fn hold_player(world: &mut World, p: PlayerId) {
    let Some(e) = character_of(world, p) else { return };
    if let Ok(mut ent) = world.get_entity_mut(e) {
        ent.remove::<CaInMatch>();
    }
}

/// A match over closes the room; after the result's time the room is
/// waiting again and the players are held out.
fn end_of_match(world: &mut World, host: &mut CaLobbyHost, now: f64) {
    let Some(a) = host.active.clone() else { return };
    if !a.went {
        return;
    }
    let over = world.resource::<CaMatch>().phase == Phase::MatchOver;
    if over && a.over_at.is_none() {
        host.lobby.closing(a.room);
        host.active.as_mut().unwrap().over_at = Some(now + host.result_secs);
    }
    if let Some(t) = a.over_at
        && now >= t
    {
        host.lobby.match_over(a.room);
        host.active = None;
        let ins: Vec<Entity> = world.query_filtered::<Entity, With<CaInMatch>>().iter(world).collect();
        for e in ins {
            world.entity_mut(e).remove::<CaInMatch>();
        }
    }
}

/// Whether the join flow must keep `e` out of the match: the lobby is on
/// and the character has not been sent in.
pub fn holds(world: &World, e: Entity) -> bool {
    world.get_resource::<CaLobbyHost>().is_some_and(|h| h.enabled) && world.get::<CaInMatch>(e).is_none()
}

/// While the lobby is on, players not sent in stay out of the match.
fn hold_outsiders(world: &mut World) {
    let mut players: Vec<Entity> = world.query::<&Player>().iter(world).map(|p| p.character).collect();
    players.extend(world.query_filtered::<Entity, With<LocalPlayer>>().iter(world));
    for e in players {
        let Ok(ent) = world.get_entity(e) else { continue };
        if !ent.contains::<CaInMatch>() && !ent.contains::<Connecting>() {
            world.entity_mut(e).insert(Connecting);
        }
    }
}

/// The players a room has in the match: for tests and the HUD.
pub fn in_match(world: &mut World) -> HashMap<Entity, u8> {
    world
        .query_filtered::<(Entity, &MatchTeam), With<CaInMatch>>()
        .iter(world)
        .map(|(e, t)| (e, t.0))
        .collect()
}
