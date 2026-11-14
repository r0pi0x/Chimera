//! RCON over the game's connection: a player on a server who knows its
//! `rcon_password` runs console lines on the server as its operator, and
//! sees the server settings an app lists for admins (the party's Escape
//! menu's Host settings, `party_kit::host`).
//!
//! - **Client**: `rcon_password <password>` (the player's own setting,
//!   never saved) logs in: sent once per connection when the game is
//!   joined or the password changes (`RconRequest` with no line). `rcon
//!   <command>` sends the command with the password (`RconRequest`); the
//!   server's output comes back as `cheats::ConsoleText`. `RconAdmin` holds
//!   whether the server took the password and the values of the settings
//!   it lists for admins (`RconStatus`); a menu reads it
//!   (`RconAdmin::is_admin`).
//! - **Server**: checks the password of every request against its own
//!   `rcon_password` (compared in constant time); empty, RCON is off. A
//!   right one marks the client `RconAdminClient` (it gets `AdminCvars`'
//!   values now and on every change) and runs the line, as the server's
//!   console would (no player). A new server password drops every admin.
//! - **Wrong passwords** (ours: the specs don't cover Source's rules
//!   here): each counts against the client's address (its client id on
//!   the memory link); at `sv_rcon_maxfailures` (5) the address is
//!   banned from RCON until the server restarts, and the client is
//!   kicked. A right password clears the count.
//! - **No passwords in logs**: the log names who logged in or failed,
//!   and a command's arguments are hidden when its name has "password" in
//!   it (`shown_line`); the console doesn't log what such cvars print
//!   (`console::execute`).
//!
//! Known shortcuts: no TCP RCON (Source's remote protocol for tools); the
//! bans live only in the server's memory.

use std::collections::{BTreeMap, BTreeSet};

use bevy::prelude::*;
use bevy_replicon::{prelude::*, shared::backend::connected_client::NetworkId};
use serde::{Deserialize, Serialize};

use super::{NetEvent, cheats::ConsoleText, server::Player};
use crate::{
    console::{Console, ConsoleAppExt, Level, execute, parse, resource_cvar},
    core::NetRole,
};

/// Client -> server: the password, and a console line to run (none: just
/// log in; none and no password: log out).
#[derive(Message, Serialize, Deserialize, Clone, Default, PartialEq)]
pub struct RconRequest {
    pub password: String,
    pub line: String,
}

impl std::fmt::Debug for RconRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RconRequest").field("line", &shown_line(&self.line)).finish_non_exhaustive()
    }
}

/// Server -> a client: whether it is an RCON admin now, and values of
/// `AdminCvars` (all on logging in, then those that changed).
#[derive(Message, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct RconStatus {
    pub admin: bool,
    pub values: Vec<(String, String)>,
}

/// `rcon_password`: on a server, the password RCON asks for (empty: off);
/// on a client, the one it gives.
#[derive(Resource, Default, Clone)]
pub struct RconPassword(pub String);

/// The server settings admins see (by the name the app reads them by):
/// an app lists them (`party_kit::host`).
#[derive(Resource, Default, Debug, Clone)]
pub struct AdminCvars(pub BTreeSet<String>);

/// RCON's rules on a server (`sv_rcon_maxfailures`).
#[derive(Resource, Debug, Clone)]
pub struct RconRules {
    /// Wrong passwords from one address before it is banned.
    pub max_failures: u32,
}

impl Default for RconRules {
    fn default() -> Self {
        Self { max_failures: 5 }
    }
}

/// On a server: wrong passwords by address, and the banned addresses.
#[derive(Resource, Default, Debug)]
pub struct RconBans {
    pub failures: BTreeMap<String, u32>,
    pub banned: BTreeSet<String>,
}

/// On a client's server-side entity: it gave the right password.
#[derive(Component, Debug)]
pub struct RconAdminClient;

/// On a client: what the server said of our RCON.
#[derive(Resource, Default, Debug, Clone)]
pub struct RconAdmin {
    pub admin: bool,
    /// `AdminCvars`' values on the server.
    pub values: BTreeMap<String, String>,
    /// The password sent on this connection (no second login with it).
    sent: Option<String>,
    joined: bool,
}

impl RconAdmin {
    /// Whether this process is a network client the server took as an
    /// admin.
    pub fn is_admin(world: &World) -> bool {
        world.get_resource::<NetRole>() == Some(&NetRole::Client)
            && world.get_resource::<RconAdmin>().is_some_and(|a| a.admin)
    }
}

/// On a server: the admin values each admin has been sent.
#[derive(Resource, Default)]
struct SentAdminValues(BTreeMap<String, String>);

/// Longest line and password a client may send.
const MAX_LINE: usize = 512;
const MAX_PASSWORD: usize = 128;

/// What a kicked client is told.
pub const BANNED: &str = "Banned from RCON: too many wrong passwords.";

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<RconPassword>()
        .init_resource::<AdminCvars>()
        .init_resource::<RconRules>()
        .init_resource::<RconBans>()
        .init_resource::<RconAdmin>()
        .init_resource::<SentAdminValues>()
        .add_systems(
            PreUpdate,
            receive_requests
                .after(ServerSystems::Receive)
                .run_if(in_state(ServerState::Running)),
        )
        .add_systems(
            PostUpdate,
            (
                drop_admins_on_new_password.run_if(resource_changed::<RconPassword>),
                send_changes.run_if(any_with_component::<RconAdminClient>),
            )
                .chain()
                .before(ServerSystems::Send)
                .run_if(resource_equals(NetRole::Server)),
        )
        .add_systems(
            PreUpdate,
            receive_status
                .after(ClientSystems::Receive)
                .run_if(resource_equals(NetRole::Client)),
        )
        .add_systems(
            PostUpdate,
            // Every role: a client's leaving makes it a standalone game.
            log_in.before(ClientSystems::Send),
        );
    resource_cvar::<RconPassword, String>(
        app,
        "rcon_password",
        "On a server, the password `rcon` asks for (empty: RCON off); on a client, the one it logs in with. Never saved.",
        |p| &mut p.0,
    );
    resource_cvar::<RconRules, u32>(
        app,
        "sv_rcon_maxfailures",
        "Wrong RCON passwords from one address before it is banned from RCON (and kicked) until the server restarts.",
        |r| &mut r.max_failures,
    );
    app.console_command(
        "rcon",
        "rcon <command>: run a command on the server you are connected to (needs its rcon_password).",
        |w, a| {
            if a.is_empty() {
                return Err("rcon <command>".into());
            }
            if w.get_resource::<NetRole>() != Some(&NetRole::Client) {
                return Err("rcon: not connected to a server".into());
            }
            let password = w.resource::<RconPassword>().0.clone();
            if password.is_empty() {
                return Err("rcon: set rcon_password first".into());
            }
            let line = w.resource::<Console>().wire_line(a);
            w.write_message(RconRequest { password, line });
            Ok(None)
        },
    );
}

/// `line` for the log: a command naming a password loses its arguments.
pub fn shown_line(line: &str) -> String {
    parse(line)
        .into_iter()
        .map(|words| match words.first() {
            Some(name) if name.to_lowercase().contains("password") => format!("{name} (hidden)"),
            _ => words.join(" "),
        })
        .collect::<Vec<_>>()
        .join("; ")
}

/// The two the same, in time that doesn't depend on where they differ.
fn same_secret(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    let mut diff = (a.len() != b.len()) as u8;
    for i in 0..a.len().max(b.len()) {
        diff |= a.get(i).copied().unwrap_or(0) ^ b.get(i).copied().unwrap_or(0);
    }
    diff == 0
}

fn receive_requests(mut requests: MessageReader<FromClient<RconRequest>>, mut commands: Commands) {
    for r in requests.read() {
        let Some(client) = r.client_id.entity() else { continue };
        if r.message.line.len() > MAX_LINE || r.message.password.len() > MAX_PASSWORD {
            continue;
        }
        let request = r.message.clone();
        commands.queue(move |w: &mut World| handle(w, client, request));
    }
}

/// Where failures count: the client's address, else its id.
fn ban_key(world: &World, client: Entity) -> String {
    let id = world.get::<NetworkId>(client).map_or(0, |n| n.get());
    match world.get_resource::<super::server::PeerTable>().and_then(|t| t.0.get(&id)) {
        Some((ip, _)) => format!("ip {ip}"),
        None => format!("id {id}"),
    }
}

fn who(world: &World, client: Entity) -> String {
    world.get::<Player>(client).map_or_else(|| format!("client {client}"), |p| p.name.clone())
}

fn tell(world: &mut World, client: Entity, lines: Vec<(String, bool)>) {
    if !lines.is_empty() {
        world.write_message(ToClients {
            targets: SendTargets::Single(ClientId::Client(client)),
            message: ConsoleText { lines },
        });
    }
}

fn status(world: &mut World, client: Entity, admin: bool, values: Vec<(String, String)>) {
    world.write_message(ToClients {
        targets: SendTargets::Single(ClientId::Client(client)),
        message: RconStatus { admin, values },
    });
}

/// One request: the password checked, then the line run.
fn handle(world: &mut World, client: Entity, request: RconRequest) {
    if world.get_entity(client).is_err() {
        return;
    }
    let name = who(world, client);
    let key = ban_key(world, client);
    if request.password.is_empty() && request.line.is_empty() {
        if world.entity_mut(client).take::<RconAdminClient>().is_some() {
            info!("{name} logged out of RCON");
        }
        status(world, client, false, Vec::new());
        return;
    }
    if world.resource::<RconBans>().banned.contains(&key) {
        tell(world, client, vec![(BANNED.into(), true)]);
        return;
    }
    let ours = world.resource::<RconPassword>().0.clone();
    if ours.is_empty() {
        tell(world, client, vec![("RCON is off on this server (no rcon_password).".into(), true)]);
        status(world, client, false, Vec::new());
        return;
    }
    if !same_secret(&request.password, &ours) {
        let max = world.resource::<RconRules>().max_failures.max(1);
        let n = {
            let mut bans = world.resource_mut::<RconBans>();
            let n = bans.failures.entry(key.clone()).or_default();
            *n += 1;
            *n
        };
        warn!("bad RCON password from {name} ({n} of {max})");
        world.entity_mut(client).remove::<RconAdminClient>();
        tell(world, client, vec![("Bad RCON password.".into(), true)]);
        status(world, client, false, Vec::new());
        if n >= max {
            world.resource_mut::<RconBans>().banned.insert(key);
            warn!("{name} banned from RCON");
            let id = world.get::<NetworkId>(client).map(|n| n.get());
            if let Some(id) = id {
                super::server::kick_userid(world, id, BANNED);
            }
        }
        return;
    }
    world.resource_mut::<RconBans>().failures.remove(&key);
    if world.get::<RconAdminClient>(client).is_none() {
        info!("{name} is an RCON admin");
        world.entity_mut(client).insert(RconAdminClient);
        let values = admin_values(world);
        world.resource_mut::<SentAdminValues>().0.extend(values.iter().cloned());
        status(world, client, true, values);
    }
    if request.line.is_empty() {
        return;
    }
    info!("rcon from {name}: {}", shown_line(&request.line));
    let before = world.resource::<Console>().printed;
    for words in parse(&request.line) {
        execute(world, &words, 0);
    }
    let console = world.resource::<Console>();
    let n = (console.printed - before) as usize;
    let out = &console.output;
    let lines = out[out.len().saturating_sub(n)..]
        .iter()
        .filter(|l| l.level != Level::Input)
        .map(|l| (l.text.clone(), l.level == Level::Error))
        .collect();
    tell(world, client, lines);
}

/// `AdminCvars`' values now.
fn admin_values(world: &mut World) -> Vec<(String, String)> {
    let names = world.resource::<AdminCvars>().0.clone();
    names
        .into_iter()
        .filter_map(|n| {
            let cvar = world.resource::<Console>().cvar(&n).cloned()?;
            Some((n, (cvar.get)(world)?))
        })
        .collect()
}

/// A new server password: nobody is an admin any more.
fn drop_admins_on_new_password(
    password: Res<RconPassword>,
    admins: Query<Entity, With<RconAdminClient>>,
    mut last: Local<Option<String>>,
    mut commands: Commands,
    mut out: MessageWriter<ToClients<RconStatus>>,
) {
    if last.as_ref() == Some(&password.0) {
        return;
    }
    let first = last.is_none();
    *last = Some(password.0.clone());
    if first {
        return;
    }
    for a in &admins {
        commands.entity(a).remove::<RconAdminClient>();
        out.write(ToClients {
            targets: SendTargets::Single(ClientId::Client(a)),
            message: RconStatus::default(),
        });
    }
}

/// Changed admin values, to the admins (reading cvars takes the world:
/// queued, only while someone is an admin).
fn send_changes(mut commands: Commands) {
    commands.queue(|world: &mut World| {
        let admins: Vec<Entity> = world.query_filtered::<Entity, With<RconAdminClient>>().iter(world).collect();
        let changed: Vec<(String, String)> = {
            let now = admin_values(world);
            let sent = &world.resource::<SentAdminValues>().0;
            now.into_iter().filter(|(k, v)| sent.get(k) != Some(v)).collect()
        };
        if changed.is_empty() {
            return;
        }
        world.resource_mut::<SentAdminValues>().0.extend(changed.iter().cloned());
        for a in admins {
            status(world, a, true, changed.clone());
        }
    });
}

fn receive_status(mut from: MessageReader<RconStatus>, mut admin: ResMut<RconAdmin>, mut console: ResMut<Console>) {
    for s in from.read() {
        if s.admin && !admin.admin {
            console.print(Level::Info, "RCON: logged in as an admin.");
        }
        admin.admin = s.admin;
        if !s.admin {
            admin.values.clear();
        }
        admin.values.extend(s.values.iter().cloned());
    }
}

/// Log in when the game is joined or `rcon_password` changes; forget it
/// all on leaving.
fn log_in(
    mut events: MessageReader<NetEvent>,
    role: Option<Res<NetRole>>,
    password: Res<RconPassword>,
    mut admin: ResMut<RconAdmin>,
    mut out: MessageWriter<RconRequest>,
) {
    for e in events.read() {
        match e {
            NetEvent::Joined { .. } => admin.joined = true,
            NetEvent::Disconnected(_) => *admin = RconAdmin::default(),
            _ => {}
        }
    }
    if role.as_deref() != Some(&NetRole::Client) || !admin.joined || admin.sent.as_deref() == Some(password.0.as_str()) {
        return;
    }
    // No password and none sent: nothing to say.
    if password.0.is_empty() && admin.sent.is_none() {
        return;
    }
    admin.sent = Some(password.0.clone());
    out.write(RconRequest {
        password: password.0.clone(),
        line: String::new(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passwords_stay_out_of_shown_lines() {
        assert_eq!(shown_line("rcon_password hunter2"), "rcon_password (hidden)");
        assert_eq!(shown_line("sv_password \"a b\"; mp_knife_finale 1"), "sv_password (hidden); mp_knife_finale 1");
        assert!(!format!("{:?}", RconRequest { password: "hunter2".into(), line: "x".into() }).contains("hunter2"));
    }

    #[test]
    fn secrets_compare() {
        assert!(same_secret("abc", "abc"));
        assert!(!same_secret("abc", "abd"));
        assert!(!same_secret("abc", "abcd"));
        assert!(!same_secret("", "a"));
    }
}
