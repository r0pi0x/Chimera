//! Server queries and LAN discovery (plan slice 8): a server answers a
//! small info request on its own UDP port, separate from the game
//! connection, so a client can list servers without joining them.
//!
//! The packets mirror Source's `A2S_INFO` as the Valve Developer Wiki's
//! "Server queries" page documents it (public): a 4-byte `FF FF FF FF`
//! header (no netcode packet starts with `FF`, so the server's socket
//! tells them apart: `udp::UdpServer`), the request `T` with
//! `"Source Engine Query\0"`, a 4-byte challenge reply `A` first (the
//! request is sent again with the challenge appended: a spoofed source
//! address gets back no more than it sent), then the info reply `I`:
//! protocol, name, map, folder, game, app id, players, max players, bots,
//! server type (`d` dedicated, `l` listen), environment (`l`, `w`, `m`),
//! visibility (1: password), VAC (0), version, and the extra data flag
//! with the port (`0x80`), an instance id in the SteamID field (`0x10`,
//! random per server process: one server reached at two addresses is
//! listed once) and keywords (`0x20`: the map's game, then
//! `minigame=<id>` while one plays, `ServerInfo::minigame`). Our servers say folder `chimera` (older builds `mashup`, also ours);
//! replies from anything else (a Source server on the LAN) are left out.
//! Answers are rate-limited per address and in total (`Responder`).
//!
//! The client side (`ServerQueries`) sends requests from its own socket,
//! measures the ping as the round trip of the challenged request, retries
//! once, and scans the LAN: the request broadcast to each port of
//! `net_lan_ports` (27015-27020, as Source's LAN tab) at the
//! `net_lan_broadcast` addresses and to this machine (127.0.0.1).
//!
//! Server queries and LAN discovery (slice 8): A2S_INFO's packets (request,
//! challenge, info: name, map, folder `chimera`, game, players, max, bots,
//! dedicated or listen, OS, password, version, port, instance id, keywords),
//! `Responder` (stateless challenges from a secret, per-address and total rate
//! limits), the server's `LocalServerInfo` kept current, `Hosting` (`hostname`,
//! `sv_password`), `JoinPassword` (`password`) and netcode user data; the
//! client's `ServerQueries` (one socket: ask a server, scan the LAN on
//! `net_lan_ports` at `net_lan_broadcast` and this machine, retry, ping by
//! round trip, one entry per server instance, other games' servers left out),
//! `serverinfo` and `lanscan`.

use std::{
    collections::HashMap,
    hash::{Hash, Hasher},
    net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use bevy::prelude::*;

use super::{NET_VERSION, NetSettings};
use crate::{
    console::{ConsoleAppExt, resource_cvar},
    core::{Intent, LocalPlayer, NetRole},
};

// The packets and the server's `Responder`: `chimera_formats::a2s` (the
// hub's probe uses them too).
pub use chimera_formats::a2s::*;

/// The game's description in the reply (the browser's Game column).
pub const GAME_NAME: &str = "Lucker Party";
/// Ports the LAN tab scans unless `net_lan_ports` says otherwise
/// (Source's: 27015-27020).
pub const LAN_PORTS: (u16, u16) = (27015, 27020);

/// A server's name and password (`hostname`, `sv_password`).
#[derive(Resource, Clone, Debug)]
pub struct Hosting {
    pub hostname: String,
    pub password: String,
}

impl Default for Hosting {
    fn default() -> Self {
        Self {
            hostname: GAME_NAME.into(),
            password: String::new(),
        }
    }
}

/// The password this client joins with (`password`, as Source's).
#[derive(Resource, Clone, Debug, Default)]
pub struct JoinPassword(pub String);

/// The bytes of netcode's user data a client sends: a tag, the
/// password's length, the password (at most 250 bytes).
pub fn user_data(password: &str) -> [u8; 256] {
    let mut out = [0u8; 256];
    out[0] = 1;
    let bytes = &password.as_bytes()[..password.len().min(250)];
    out[1] = bytes.len() as u8;
    out[2..2 + bytes.len()].copy_from_slice(bytes);
    out
}

/// The password in a client's user data ("" without).
pub fn password_of(user_data: Option<&[u8; 256]>) -> String {
    match user_data {
        Some(d) if d[0] == 1 => {
            let n = (d[1] as usize).min(250);
            String::from_utf8_lossy(&d[2..2 + n]).into_owned()
        }
        _ => String::new(),
    }
}

/// This server's info as queries get it, kept current while serving.
#[derive(Resource, Clone, Debug, Default)]
pub struct LocalServerInfo(pub ServerInfo);

/// This process's server instance id (random, per process).
#[derive(Resource, Clone, Copy, Debug)]
pub struct InstanceId(pub u64);

impl Default for InstanceId {
    fn default() -> Self {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        (std::process::id(), Instant::now(), std::time::SystemTime::now()).hash(&mut h);
        Self(h.finish())
    }
}

/// The server's info now: name, map, players and bots, its limits.
fn update_info(
    role: Res<NetRole>,
    settings: Res<NetSettings>,
    hosting: Res<Hosting>,
    instance: Res<InstanceId>,
    characters: Query<
        (Has<crate::core::Bot>, Has<LocalPlayer>),
        (With<Intent>, Without<crate::core::Npc>),
    >,
    map: Option<Res<crate::map::LoadedMapName>>,
    port: Option<Res<super::udp::UdpServer>>,
    minigame: Option<Res<chimera_sim::minigame::ActiveMinigame>>,
    mut info: ResMut<LocalServerInfo>,
) {
    if *role != NetRole::Server {
        return;
    }
    let (mut players, mut bots, mut host) = (0u32, 0u32, false);
    for (bot, local) in &characters {
        players += 1;
        bots += bot as u32;
        host |= local;
    }
    let name = map.as_ref().map(|m| m.0.clone()).unwrap_or_default();
    let map = super::normalize_map(&name);
    let next = ServerInfo {
        protocol: PROTOCOL,
        name: hosting.hostname.clone(),
        map: map.rsplit(':').next().unwrap_or(&map).to_string(),
        folder: GAME_FOLDER.into(),
        game: GAME_NAME.into(),
        app_id: 0,
        players: players.min(255) as u8,
        max_players: settings.maxplayers.min(255) as u8,
        bots: bots.min(255) as u8,
        dedicated: !host,
        os: if cfg!(windows) {
            'w'
        } else if cfg!(target_os = "macos") {
            'm'
        } else {
            'l'
        },
        password: !hosting.password.is_empty(),
        secure: false,
        version: NET_VERSION.into(),
        port: port.and_then(|p| p.local_addr()).map(|a| a.port()),
        instance: Some(instance.0),
        keywords: Some({
            let game = map.split(':').next().filter(|g| *g != map).unwrap_or("chimera");
            match &minigame {
                Some(m) => format!("{game},minigame={}", m.def.id),
                None => game.to_string(),
            }
        }),
    };
    if info.0 != next {
        info.0 = next;
    }
}

// ---------------------------------------------------------------------------
// The client side.

/// How long to wait for a reply before asking again, and how many times
/// to ask.
pub const QUERY_TIMEOUT: Duration = Duration::from_millis(1200);
pub const QUERY_TRIES: u8 = 2;
/// How long a LAN scan listens for servers.
pub const LAN_SCAN_TIME: Duration = Duration::from_millis(1500);

/// A server's state in the list.
#[derive(Clone, Debug, PartialEq)]
pub enum QueryState {
    /// Asked, no answer yet.
    Pending,
    /// It answered: its info and the ping, ms.
    Answered { info: ServerInfo, ping_ms: u32 },
    /// No answer after every try.
    NoResponse,
}

/// A server queried (or found on the LAN).
#[derive(Clone, Debug, PartialEq)]
pub struct QueryResult {
    pub addr: SocketAddr,
    pub state: QueryState,
    /// Found by a LAN scan (not asked for by address).
    pub lan: bool,
}

#[derive(Clone, Debug)]
struct Asking {
    /// When the last request went out (the ping is from it).
    sent: Instant,
    tries: u8,
    challenge: Option<i32>,
}

/// A LAN scan under way.
#[derive(Clone, Debug)]
struct Scan {
    ports: (u16, u16),
    until: Instant,
}

/// The client's queries: one socket, servers asked and their answers
/// (`results`, by address). `query` asks one server, `scan_lan` asks
/// every server on the local network; `poll` (each frame) reads replies,
/// asks again on a challenge or a timeout.
#[derive(Resource, Default)]
pub struct ServerQueries {
    socket: Option<UdpSocket>,
    /// Replies a reader thread took off the socket, with when they came
    /// (the ping doesn't wait for the next frame).
    received: Arc<Mutex<Vec<(Instant, SocketAddr, Vec<u8>)>>>,
    stop: Arc<AtomicBool>,
    asking: HashMap<SocketAddr, Asking>,
    scan: Option<Scan>,
    pub results: Vec<QueryResult>,
    /// Bumped whenever `results` change (the browser redraws).
    pub generation: u64,
    /// Why the last send failed (no network), for the status line.
    pub error: Option<String>,
}

impl Drop for ServerQueries {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

impl ServerQueries {
    fn socket(&mut self) -> Result<&UdpSocket, String> {
        if self.socket.is_none() {
            let s = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).map_err(|e| format!("UDP socket: {e}"))?;
            s.set_broadcast(true).map_err(|e| e.to_string())?;
            s.set_read_timeout(Some(Duration::from_millis(100)))
                .map_err(|e| e.to_string())?;
            let reader = s.try_clone().map_err(|e| e.to_string())?;
            let (received, stop) = (self.received.clone(), self.stop.clone());
            std::thread::Builder::new()
                .name("server queries".into())
                .spawn(move || {
                    let mut buf = [0u8; 1400];
                    while !stop.load(Ordering::Relaxed) {
                        match reader.recv_from(&mut buf) {
                            Ok((n, from)) => {
                                if let Ok(mut r) = received.lock() {
                                    r.push((Instant::now(), from, buf[..n].to_vec()));
                                }
                            }
                            Err(e)
                                if matches!(
                                    e.kind(),
                                    std::io::ErrorKind::WouldBlock
                                        | std::io::ErrorKind::TimedOut
                                        | std::io::ErrorKind::Interrupted
                                        // Windows: an earlier send's "port unreachable".
                                        | std::io::ErrorKind::ConnectionReset
                                ) => {}
                            Err(e) => {
                                debug!("server queries: {e}");
                                std::thread::sleep(Duration::from_millis(50));
                            }
                        }
                    }
                })
                .map_err(|e| e.to_string())?;
            self.socket = Some(s);
        }
        Ok(self.socket.as_ref().unwrap())
    }

    fn send(&mut self, to: SocketAddr, packet: &[u8]) {
        let r = self
            .socket()
            .and_then(|s| s.send_to(packet, to).map_err(|e| format!("{to}: {e}")));
        if let Err(e) = r {
            debug!("server query: {e}");
            self.error = Some(e);
        }
    }

    /// The entry for `addr` (made pending if new).
    fn entry(&mut self, addr: SocketAddr, lan: bool) -> &mut QueryResult {
        let i = match self.results.iter().position(|r| r.addr == addr) {
            Some(i) => i,
            None => {
                self.results.push(QueryResult {
                    addr,
                    state: QueryState::Pending,
                    lan,
                });
                self.results.len() - 1
            }
        };
        &mut self.results[i]
    }

    /// Ask a server for its info (again, if asked before).
    pub fn query(&mut self, addr: SocketAddr) {
        self.ask(addr, false, Instant::now());
    }

    fn ask(&mut self, addr: SocketAddr, lan: bool, now: Instant) {
        // What it said last stays shown until it answers again.
        let e = self.entry(addr, lan);
        if !matches!(e.state, QueryState::Answered { .. }) {
            e.state = QueryState::Pending;
        }
        e.lan |= lan;
        self.asking.insert(
            addr,
            Asking {
                sent: now,
                tries: 1,
                challenge: None,
            },
        );
        self.send(addr, &info_request(None));
        self.generation += 1;
    }

    /// Ask every server on the local network: the request broadcast to
    /// each port in `ports` at each of `broadcast`, and sent to this
    /// machine's. Servers found before are forgotten.
    pub fn scan_lan(&mut self, ports: (u16, u16), broadcast: &[IpAddr]) {
        let now = Instant::now();
        self.results.retain(|r| !r.lan);
        self.error = None;
        self.scan = Some(Scan {
            ports,
            until: now + LAN_SCAN_TIME,
        });
        let request = info_request(None);
        for port in ports.0..=ports.1.max(ports.0) {
            for ip in broadcast.iter().chain(&[IpAddr::V4(Ipv4Addr::LOCALHOST)]) {
                self.send(SocketAddr::new(*ip, port), &request);
            }
        }
        self.generation += 1;
    }

    /// Whether a LAN scan is still listening.
    pub fn scanning(&self) -> bool {
        self.scan.is_some()
    }

    /// Whether a LAN scan is still listening, or a server still asked.
    pub fn busy(&self) -> bool {
        self.scan.is_some() || !self.asking.is_empty()
    }

    /// Forget a server (and stop asking it).
    pub fn forget(&mut self, addr: SocketAddr) {
        self.asking.remove(&addr);
        self.results.retain(|r| r.addr != addr);
        self.generation += 1;
    }

    pub fn result(&self, addr: SocketAddr) -> Option<&QueryResult> {
        self.results.iter().find(|r| r.addr == addr)
    }

    /// Read replies, answer challenges, ask again or give up on servers
    /// that don't answer.
    pub fn poll(&mut self) {
        let replies = self
            .received
            .lock()
            .map(|mut r| std::mem::take(&mut *r))
            .unwrap_or_default();
        for (at, from, packet) in replies {
            self.reply(from, &packet, at);
        }
        let now = Instant::now();
        // Timeouts.
        let mut again = Vec::new();
        let mut gave_up = Vec::new();
        for (addr, a) in &self.asking {
            if now.duration_since(a.sent) < QUERY_TIMEOUT {
                continue;
            }
            if a.tries < QUERY_TRIES {
                again.push(*addr);
            } else {
                gave_up.push(*addr);
            }
        }
        for addr in again {
            let challenge = {
                let a = self.asking.get_mut(&addr).unwrap();
                a.tries += 1;
                a.sent = now;
                a.challenge
            };
            self.send(addr, &info_request(challenge));
        }
        for addr in gave_up {
            self.asking.remove(&addr);
            let lan = self.result(addr).is_some_and(|r| r.lan);
            if lan {
                // A LAN server that went quiet: not listed.
                self.results.retain(|r| r.addr != addr);
            } else {
                self.entry(addr, false).state = QueryState::NoResponse;
            }
            self.generation += 1;
        }
        if self.scan.as_ref().is_some_and(|s| now >= s.until) {
            self.scan = None;
            self.generation += 1;
        }
    }

    fn reply(&mut self, from: SocketAddr, packet: &[u8], now: Instant) {
        let Some(reply) = parse_reply(packet) else { return };
        let scanning = self
            .scan
            .as_ref()
            .is_some_and(|s| (s.ports.0..=s.ports.1).contains(&from.port()));
        if !self.asking.contains_key(&from) {
            // A server answering the LAN broadcast: ask it as any other.
            if !(scanning && matches!(reply, Reply::Challenge(_))) {
                return;
            }
            if self
                .result(from)
                .is_some_and(|r| matches!(r.state, QueryState::Answered { .. }))
            {
                return;
            }
            self.entry(from, true);
            self.asking.insert(
                from,
                Asking {
                    sent: now,
                    tries: 1,
                    challenge: None,
                },
            );
        }
        match reply {
            Reply::Challenge(c) => {
                let a = self.asking.get_mut(&from).unwrap();
                a.challenge = Some(c);
                // The round trip is from this send.
                a.sent = Instant::now();
                self.send(from, &info_request(Some(c)));
            }
            Reply::Info(info) => {
                let a = self.asking.remove(&from).unwrap();
                let lan = self.result(from).is_some_and(|r| r.lan);
                if lan && !info.is_ours() {
                    // Another game's server on a scanned port.
                    self.results.retain(|r| r.addr != from);
                    self.generation += 1;
                    return;
                }
                // One server at two addresses (its LAN address and
                // loopback): listed once, at the LAN address.
                if lan
                    && let Some(id) = info.instance
                    && let Some(other) = self.results.iter().position(|r| {
                        r.addr != from
                            && r.lan
                            && matches!(&r.state, QueryState::Answered { info, .. } if info.instance == Some(id))
                    })
                {
                    if from.ip().is_loopback() {
                        self.results.retain(|r| r.addr != from);
                        self.generation += 1;
                        return;
                    }
                    self.results.remove(other);
                }
                let ping_ms = now.saturating_duration_since(a.sent).as_millis().min(9999) as u32;
                self.entry(from, lan).state = QueryState::Answered { info, ping_ms };
                self.generation += 1;
            }
        }
    }
}

/// `net_lan_ports` and `net_lan_broadcast`.
#[derive(Resource, Clone, Debug)]
pub struct LanSettings {
    /// `first-last` or one port.
    pub ports: String,
    /// Broadcast addresses, space separated.
    pub broadcast: String,
}

impl Default for LanSettings {
    fn default() -> Self {
        Self {
            ports: format!("{}-{}", LAN_PORTS.0, LAN_PORTS.1),
            broadcast: "255.255.255.255".into(),
        }
    }
}

impl LanSettings {
    /// The port range (Source's when unreadable).
    pub fn port_range(&self) -> (u16, u16) {
        let t = self.ports.trim();
        let parse = |s: &str| s.trim().parse::<u16>().ok();
        let range = match t.split_once('-') {
            Some((a, b)) => parse(a).zip(parse(b)),
            None => parse(t).map(|p| (p, p)),
        };
        match range {
            Some((a, b)) if a > 0 && b >= a && b - a <= 64 => (a, b),
            _ => LAN_PORTS,
        }
    }

    pub fn broadcast_addresses(&self) -> Vec<IpAddr> {
        self.broadcast
            .split_whitespace()
            .filter_map(|s| s.parse().ok())
            .collect()
    }
}

/// One line about a server, for the console.
pub fn describe(r: &QueryResult) -> String {
    match &r.state {
        QueryState::Pending => format!("{}  (asking...)", r.addr),
        QueryState::NoResponse => format!("{}  < not responding >", r.addr),
        QueryState::Answered { info, ping_ms } => format!(
            "{}  \"{}\"  {}  {}/{} players ({} bots)  {} ms  {}{}{}",
            r.addr,
            info.name,
            info.map,
            info.players,
            info.max_players,
            info.bots,
            ping_ms,
            if info.dedicated { "dedicated" } else { "listen" },
            if info.password { ", password" } else { "" },
            if info.version != NET_VERSION {
                format!(", version {}", info.version)
            } else {
                String::new()
            }
        ),
    }
}

/// Pending console printouts of `serverinfo`/`lanscan`: printed once the
/// queries settle.
#[derive(Resource, Default)]
struct ConsoleQuery {
    /// Addresses asked by `serverinfo` (None: the LAN scan's results).
    addrs: Option<Vec<SocketAddr>>,
    waiting: bool,
}

fn poll(mut queries: ResMut<ServerQueries>) {
    if queries.socket.is_some() {
        queries.poll();
    }
}

fn print_console_query(
    mut pending: ResMut<ConsoleQuery>,
    queries: Res<ServerQueries>,
    mut console: ResMut<crate::console::Console>,
) {
    if !pending.waiting || queries.busy() {
        return;
    }
    pending.waiting = false;
    let lines: Vec<String> = match &pending.addrs {
        Some(addrs) => addrs.iter().filter_map(|a| queries.result(*a)).map(describe).collect(),
        None => {
            let found: Vec<String> = queries.results.iter().filter(|r| r.lan).map(describe).collect();
            if found.is_empty() {
                vec!["No servers on the local network.".into()]
            } else {
                found
            }
        }
    };
    for l in lines {
        console.info(l);
    }
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<Hosting>()
        .init_resource::<JoinPassword>()
        .init_resource::<LocalServerInfo>()
        .init_resource::<InstanceId>()
        .init_resource::<ServerQueries>()
        .init_resource::<LanSettings>()
        .init_resource::<ConsoleQuery>()
        .add_systems(PreUpdate, update_info.before(bevy_replicon_renet::RenetReceive))
        .add_systems(Update, (poll, print_console_query).chain());
    resource_cvar::<Hosting, String>(app, "hostname", "The server's name in server lists.", |h| {
        &mut h.hostname
    });
    resource_cvar::<Hosting, String>(
        app,
        "sv_password",
        "Players must give this password to join (`password`); empty: none.",
        |h| &mut h.password,
    );
    resource_cvar::<JoinPassword, String>(app, "password", "The password to join a server with.", |p| &mut p.0);
    resource_cvar::<LanSettings, String>(
        app,
        "chimera:lan_ports",
        "UDP ports the LAN tab and `lanscan` look for servers on: first-last (Source's 27015-27020).",
        |l| &mut l.ports,
    );
    resource_cvar::<LanSettings, String>(
        app,
        "chimera:lan_broadcast",
        "Addresses the LAN scan broadcasts to, space separated (this machine is always asked too).",
        |l| &mut l.broadcast,
    );
    app.console_command(
        "chimera:serverinfo",
        "serverinfo <ip[:port]> ...: ask servers for their name, map, players and ping.",
        |w, a| {
            if a.is_empty() {
                return Err("serverinfo <ip[:port]> ...".into());
            }
            let addrs = a
                .iter()
                .map(|s| super::parse_address(s))
                .collect::<Result<Vec<_>, _>>()?;
            let mut q = w.resource_mut::<ServerQueries>();
            for addr in &addrs {
                q.query(*addr);
            }
            *w.resource_mut::<ConsoleQuery>() = ConsoleQuery {
                addrs: Some(addrs),
                waiting: true,
            };
            Ok(Some("Asking...".into()))
        },
    )
    .console_command(
        "chimera:lanscan",
        "List servers on the local network (`net_lan_ports`).",
        |w, _| {
            let lan = w.resource::<LanSettings>().clone();
            let (ports, broadcast) = (lan.port_range(), lan.broadcast_addresses());
            w.resource_mut::<ServerQueries>().scan_lan(ports, &broadcast);
            *w.resource_mut::<ConsoleQuery>() = ConsoleQuery {
                addrs: None,
                waiting: true,
            };
            Ok(Some(format!(
                "Looking for servers on UDP ports {}-{}...",
                ports.0, ports.1
            )))
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passwords_ride_in_user_data() {
        assert_eq!(password_of(Some(&user_data("hunter2"))), "hunter2");
        assert_eq!(password_of(Some(&user_data(""))), "");
        assert_eq!(password_of(None), "");
        assert_eq!(password_of(Some(&[0; 256])), "");
        let long = "x".repeat(400);
        assert_eq!(password_of(Some(&user_data(&long))).len(), 250);
    }

    #[test]
    fn lan_port_ranges() {
        let mut l = LanSettings::default();
        assert_eq!(l.port_range(), (27015, 27020));
        l.ports = "27031-27032".into();
        assert_eq!(l.port_range(), (27031, 27032));
        l.ports = "27040".into();
        assert_eq!(l.port_range(), (27040, 27040));
        l.ports = "9-2".into();
        assert_eq!(l.port_range(), LAN_PORTS);
        l.broadcast = "255.255.255.255 192.168.1.255 junk".into();
        assert_eq!(l.broadcast_addresses().len(), 2);
    }
}
