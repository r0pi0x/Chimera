//! The A2S server query packets (Source's `A2S_INFO` as the Valve
//! Developer Wiki's public "Server queries" page documents it), lifted
//! from `chimera_net::net::query` (which re-exports them) so the hub's
//! reachability probe (`chimera_services::hub`) asks a game server without
//! the network crate (docs/plans/active/hub.md, 2.7).
//!
//! A 4-byte `FF FF FF FF` header (no netcode packet starts with `FF`), the
//! request `T` with `"Source Engine Query\0"`, a 4-byte challenge reply
//! `A` first (the request is sent again with the challenge appended: a
//! spoofed source address gets back no more than it sent), then the info
//! reply `I`: protocol, name, map, folder, game, app id, players, max
//! players, bots, server type (`d` dedicated, `l` listen), environment
//! (`l`, `w`, `m`), visibility (1: password), VAC (0), version, and the
//! extra data flag with the port (`0x80`), an instance id in the SteamID
//! field (`0x10`) and keywords (`0x20`). Our servers say folder `chimera`
//! (older builds `mashup`). `Responder` answers with stateless challenges
//! from a secret and rate limits per address and in total;
//! `query_blocking` asks one server from a plain socket (the probe).

use std::{
    collections::HashMap,
    hash::{Hash, Hasher},
    net::{IpAddr, SocketAddr},
    time::Duration,
};

/// Every query packet starts with this (a connectionless packet in
/// Source's terms).
pub const HEADER: [u8; 4] = [0xFF; 4];
/// Request: server info.
pub const A2S_INFO: u8 = b'T';
/// Reply: server info.
pub const S2A_INFO: u8 = b'I';
/// Reply: send the request again with this challenge.
pub const S2C_CHALLENGE: u8 = b'A';
/// The info request's payload.
pub const INFO_PAYLOAD: &[u8] = b"Source Engine Query\0";
/// The protocol byte of the info reply (Source's).
pub const PROTOCOL: u8 = 17;
/// Our servers' game folder: replies naming another are not ours.
pub const GAME_FOLDER: &str = "chimera";
/// Its name before the rename to Chimera (servers of older builds).
pub const OLD_GAME_FOLDER: &str = "mashup";

/// Extra data flags of the info reply.
mod edf {
    pub const PORT: u8 = 0x80;
    pub const STEAM_ID: u8 = 0x10;
    pub const KEYWORDS: u8 = 0x20;
    pub const SOURCE_TV: u8 = 0x40;
    pub const GAME_ID: u8 = 0x01;
}

/// What a server says about itself.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ServerInfo {
    pub protocol: u8,
    pub name: String,
    pub map: String,
    pub folder: String,
    pub game: String,
    pub app_id: u16,
    /// Players in the game, bots included (as Source counts them).
    pub players: u8,
    pub max_players: u8,
    pub bots: u8,
    pub dedicated: bool,
    /// `l` Linux, `w` Windows, `m` macOS.
    pub os: char,
    pub password: bool,
    pub secure: bool,
    pub version: String,
    /// The game port (the same as the query port for ours).
    pub port: Option<u16>,
    /// Random per server process (Source's SteamID field).
    pub instance: Option<u64>,
    pub keywords: Option<String>,
}

impl ServerInfo {
    /// Whether it's one of ours (not a Source server that answered).
    pub fn is_ours(&self) -> bool {
        self.folder.eq_ignore_ascii_case(GAME_FOLDER) || self.folder.eq_ignore_ascii_case(OLD_GAME_FOLDER)
    }

    /// The minigame it plays (its keywords' `minigame=<id>`), if any.
    pub fn minigame(&self) -> Option<&str> {
        self.keywords.as_deref()?.split(',').find_map(|k| k.trim().strip_prefix("minigame="))
    }

    /// The info reply.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = HEADER.to_vec();
        out.push(S2A_INFO);
        out.push(self.protocol);
        for s in [&self.name, &self.map, &self.folder, &self.game] {
            put_str(&mut out, s);
        }
        out.extend_from_slice(&self.app_id.to_le_bytes());
        out.extend_from_slice(&[
            self.players,
            self.max_players,
            self.bots,
            if self.dedicated { b'd' } else { b'l' },
            self.os as u8,
            self.password as u8,
            self.secure as u8,
        ]);
        put_str(&mut out, &self.version);
        let mut flags = 0;
        if self.port.is_some() {
            flags |= edf::PORT;
        }
        if self.instance.is_some() {
            flags |= edf::STEAM_ID;
        }
        if self.keywords.is_some() {
            flags |= edf::KEYWORDS;
        }
        out.push(flags);
        if let Some(p) = self.port {
            out.extend_from_slice(&p.to_le_bytes());
        }
        if let Some(id) = self.instance {
            out.extend_from_slice(&id.to_le_bytes());
        }
        if let Some(k) = &self.keywords {
            put_str(&mut out, k);
        }
        out
    }
}

/// A string as the protocol writes it: its bytes (no NUL inside) and a
/// NUL, at most 255 bytes.
fn put_str(out: &mut Vec<u8>, s: &str) {
    let mut n = 0;
    for c in s.chars().filter(|c| *c != '\0') {
        if n + c.len_utf8() > 255 {
            break;
        }
        let mut b = [0; 4];
        out.extend_from_slice(c.encode_utf8(&mut b).as_bytes());
        n += c.len_utf8();
    }
    out.push(0);
}

/// Reads a reply front to back.
struct Reader<'a>(&'a [u8]);

impl Reader<'_> {
    fn u8(&mut self) -> Option<u8> {
        let (&b, rest) = self.0.split_first()?;
        self.0 = rest;
        Some(b)
    }

    fn bytes<const N: usize>(&mut self) -> Option<[u8; N]> {
        let b: [u8; N] = self.0.get(..N)?.try_into().ok()?;
        self.0 = &self.0[N..];
        Some(b)
    }

    fn str(&mut self) -> Option<String> {
        let end = self.0.iter().position(|&b| b == 0)?;
        let s = String::from_utf8_lossy(&self.0[..end]).into_owned();
        self.0 = &self.0[end + 1..];
        Some(s)
    }
}

/// The info request, with the challenge the server gave (None: the first).
pub fn info_request(challenge: Option<i32>) -> Vec<u8> {
    let mut out = HEADER.to_vec();
    out.push(A2S_INFO);
    out.extend_from_slice(INFO_PAYLOAD);
    if let Some(c) = challenge {
        out.extend_from_slice(&c.to_le_bytes());
    }
    out
}

/// Whether a packet is a query (not netcode's).
pub fn is_query(packet: &[u8]) -> bool {
    packet.starts_with(&HEADER)
}

/// A request a server understands: the info request and its challenge
/// (None without one, or `-1`, Source's "none yet").
pub fn parse_request(packet: &[u8]) -> Option<Option<i32>> {
    let rest = packet.strip_prefix(&HEADER)?;
    let rest = rest.strip_prefix(&[A2S_INFO])?;
    let rest = rest.strip_prefix(INFO_PAYLOAD)?;
    match rest.len() {
        0 => Some(None),
        4.. => {
            let c = i32::from_le_bytes(rest[..4].try_into().ok()?);
            Some((c != -1).then_some(c))
        }
        _ => None,
    }
}

/// A reply a client understands.
#[derive(Clone, Debug, PartialEq)]
pub enum Reply {
    Challenge(i32),
    Info(ServerInfo),
}

pub fn challenge_reply(challenge: i32) -> Vec<u8> {
    let mut out = HEADER.to_vec();
    out.push(S2C_CHALLENGE);
    out.extend_from_slice(&challenge.to_le_bytes());
    out
}

pub fn parse_reply(packet: &[u8]) -> Option<Reply> {
    let rest = packet.strip_prefix(&HEADER)?;
    let mut r = Reader(rest);
    match r.u8()? {
        S2C_CHALLENGE => Some(Reply::Challenge(i32::from_le_bytes(r.bytes()?))),
        S2A_INFO => {
            let mut info = ServerInfo {
                protocol: r.u8()?,
                name: r.str()?,
                map: r.str()?,
                folder: r.str()?,
                game: r.str()?,
                app_id: u16::from_le_bytes(r.bytes()?),
                players: r.u8()?,
                max_players: r.u8()?,
                bots: r.u8()?,
                dedicated: r.u8()? == b'd',
                os: r.u8()? as char,
                password: r.u8()? != 0,
                secure: r.u8()? != 0,
                version: r.str()?,
                ..Default::default()
            };
            // The extra data is optional (old servers stop here).
            if let Some(flags) = r.u8() {
                if flags & edf::PORT != 0 {
                    info.port = Some(u16::from_le_bytes(r.bytes()?));
                }
                if flags & edf::STEAM_ID != 0 {
                    info.instance = Some(u64::from_le_bytes(r.bytes()?));
                }
                if flags & edf::SOURCE_TV != 0 {
                    let _port: [u8; 2] = r.bytes()?;
                    let _name = r.str()?;
                }
                if flags & edf::KEYWORDS != 0 {
                    info.keywords = Some(r.str()?);
                }
                if flags & edf::GAME_ID != 0 {
                    let _id: [u8; 8] = r.bytes()?;
                }
            }
            Some(Reply::Info(info))
        }
        _ => None,
    }
}

/// A token bucket: `rate` a second, at most `burst` saved up.
#[derive(Clone, Copy, Debug)]
struct Bucket {
    tokens: f32,
    at: Duration,
}

impl Bucket {
    fn take(&mut self, now: Duration, rate: f32, burst: f32) -> bool {
        let dt = now.saturating_sub(self.at).as_secs_f32();
        self.tokens = (self.tokens + dt * rate).min(burst);
        self.at = now;
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

/// Answers per address a second (and saved up); in total.
pub const PER_ADDRESS: (f32, f32) = (8.0, 16.0);
pub const TOTAL: (f32, f32) = (200.0, 400.0);
/// Addresses remembered for their buckets before the oldest are forgotten.
const MAX_ADDRESSES: usize = 4096;
/// How long a challenge holds, s (the current window and the one before).
const CHALLENGE_WINDOW: u64 = 30;

/// A server's query answers: challenges from a secret (nothing kept per
/// request), rate limits, counts (`status`).
#[derive(Debug)]
pub struct Responder {
    secret: u64,
    buckets: HashMap<IpAddr, Bucket>,
    total: Bucket,
    /// Info replies sent, challenges sent, requests dropped (rate limit or
    /// malformed).
    pub answered: u64,
    pub challenged: u64,
    pub dropped: u64,
}

impl Responder {
    pub fn new(secret: u64) -> Self {
        Self {
            secret,
            buckets: HashMap::new(),
            total: Bucket {
                tokens: TOTAL.1,
                at: Duration::ZERO,
            },
            answered: 0,
            challenged: 0,
            dropped: 0,
        }
    }

    /// The challenge for an address in a time window.
    fn challenge(&self, from: SocketAddr, window: u64) -> i32 {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        (self.secret, from, window).hash(&mut h);
        // Never -1 (Source's "no challenge yet").
        let c = h.finish() as i32;
        if c == -1 { 0 } else { c }
    }

    /// The answer to a query packet from `from` at `now` (a monotonic
    /// clock): a challenge, the info, or nothing (not a request, or over
    /// the rate limits).
    pub fn handle(&mut self, from: SocketAddr, packet: &[u8], now: Duration, info: &ServerInfo) -> Option<Vec<u8>> {
        let Some(challenge) = parse_request(packet) else {
            self.dropped += 1;
            return None;
        };
        if self.buckets.len() >= MAX_ADDRESSES && !self.buckets.contains_key(&from.ip()) {
            // Forget those that have been quiet longest.
            let mut ats: Vec<Duration> = self.buckets.values().map(|b| b.at).collect();
            ats.sort();
            let cut = ats[ats.len() / 2];
            self.buckets.retain(|_, b| b.at > cut);
        }
        let bucket = self.buckets.entry(from.ip()).or_insert(Bucket {
            tokens: PER_ADDRESS.1,
            at: now,
        });
        if !bucket.take(now, PER_ADDRESS.0, PER_ADDRESS.1) || !self.total.take(now, TOTAL.0, TOTAL.1) {
            self.dropped += 1;
            return None;
        }
        let window = now.as_secs() / CHALLENGE_WINDOW;
        let valid = challenge.is_some_and(|c| {
            c == self.challenge(from, window) || (window > 0 && c == self.challenge(from, window - 1))
        });
        if valid {
            self.answered += 1;
            Some(info.encode())
        } else {
            self.challenged += 1;
            Some(challenge_reply(self.challenge(from, window)))
        }
    }
}

/// Ask `addr` for its info once, blocking, from `socket` (bound, no
/// timeout set needed): the request, the challenge, the request again;
/// at most `wait` in all. None: no answer (or not an A2S server). The
/// hub's reachability probe; replies from any other address are ignored.
pub fn query_blocking(socket: &std::net::UdpSocket, addr: SocketAddr, wait: Duration) -> Option<ServerInfo> {
    let until = std::time::Instant::now() + wait;
    let mut challenge = None;
    for _ in 0..3 {
        socket.send_to(&info_request(challenge), addr).ok()?;
        let mut buf = [0u8; 1400];
        let reply = loop {
            let left = until.checked_duration_since(std::time::Instant::now())?;
            socket.set_read_timeout(Some(left.max(Duration::from_millis(1)))).ok()?;
            match socket.recv_from(&mut buf) {
                Ok((n, from)) if from == addr => break parse_reply(&buf[..n]),
                Ok(_) => continue,
                Err(e) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => return None,
                // A closed port on this machine answers with an ICMP
                // error that some systems report here: no server.
                Err(_) => return None,
            }
        };
        match reply? {
            Reply::Info(info) => return Some(info),
            Reply::Challenge(c) => challenge = Some(c),
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use super::*;

    const VERSION: &str = "0.1.0/net54";

    fn info() -> ServerInfo {
        ServerInfo {
            protocol: PROTOCOL,
            name: "Test server".into(),
            map: "de_dust2".into(),
            folder: GAME_FOLDER.into(),
            game: "Lucker Party".into(),
            app_id: 0,
            players: 3,
            max_players: 8,
            bots: 2,
            dedicated: true,
            os: 'l',
            password: true,
            secure: false,
            version: VERSION.into(),
            port: Some(27031),
            instance: Some(0x1122_3344_5566_7788),
            keywords: Some("cs_source".into()),
        }
    }

    #[test]
    fn info_reply_round_trips() {
        let i = info();
        assert_eq!(parse_reply(&i.encode()), Some(Reply::Info(i.clone())));
        let bare = ServerInfo {
            port: None,
            instance: None,
            keywords: None,
            ..i
        };
        assert_eq!(parse_reply(&bare.encode()), Some(Reply::Info(bare)));
    }

    #[test]
    fn the_keywords_carry_the_minigame() {
        let mut i = info();
        assert_eq!(i.minigame(), None);
        i.keywords = Some("cs_source,minigame=ultimate_gun_game:auto".into());
        let back = match parse_reply(&i.encode()) {
            Some(Reply::Info(b)) => b,
            other => panic!("{other:?}"),
        };
        assert_eq!(back.minigame(), Some("ultimate_gun_game:auto"));
    }

    #[test]
    fn the_layout_is_a2s_info() {
        // The documented byte layout, front to back.
        let mut i = info();
        i.instance = None;
        i.keywords = None;
        let b = i.encode();
        assert_eq!(&b[..6], &[0xFF, 0xFF, 0xFF, 0xFF, b'I', 17]);
        let mut at = 6;
        for s in ["Test server", "de_dust2", "chimera", "Lucker Party"] {
            assert_eq!(&b[at..at + s.len()], s.as_bytes());
            assert_eq!(b[at + s.len()], 0);
            at += s.len() + 1;
        }
        assert_eq!(&b[at..at + 9], &[0, 0, 3, 8, 2, b'd', b'l', 1, 0]);
        at += 9;
        at += VERSION.len() + 1;
        assert_eq!(b[at], 0x80, "the extra data flag: port");
        assert_eq!(&b[at + 1..], &27031u16.to_le_bytes());
    }

    #[test]
    fn requests_and_challenges() {
        let r = info_request(None);
        assert_eq!(&r[..5], &[0xFF, 0xFF, 0xFF, 0xFF, b'T']);
        assert_eq!(&r[5..], b"Source Engine Query\0");
        assert_eq!(parse_request(&r), Some(None));
        assert_eq!(parse_request(&info_request(Some(-1))), Some(None));
        assert_eq!(parse_request(&info_request(Some(1234))), Some(Some(1234)));
        assert_eq!(parse_request(b"\xFF\xFF\xFF\xFFTSource"), None);
        assert_eq!(parse_reply(&challenge_reply(-5)), Some(Reply::Challenge(-5)));
        assert!(parse_reply(b"\xFF\xFF\xFF\xFFI\x11abc").is_none(), "truncated");
        assert!(is_query(&r) && !is_query(&[0x01, 0xFF, 0xFF, 0xFF]));
    }

    #[test]
    fn info_only_with_the_right_challenge() {
        let mut r = Responder::new(42);
        let from: SocketAddr = "10.0.0.5:50000".parse().unwrap();
        let t = Duration::from_secs(100);
        let Some(Reply::Challenge(c)) = r
            .handle(from, &info_request(None), t, &info())
            .and_then(|b| parse_reply(&b))
        else {
            panic!("a challenge first");
        };
        // A challenge reply is never bigger than the request.
        assert!(challenge_reply(c).len() <= info_request(None).len());
        // A wrong one: challenged again.
        let wrong = r.handle(from, &info_request(Some(c ^ 1)), t, &info()).unwrap();
        assert!(matches!(parse_reply(&wrong), Some(Reply::Challenge(_))));
        // Another address can't use it.
        let other: SocketAddr = "10.0.0.6:50000".parse().unwrap();
        let theirs = r.handle(other, &info_request(Some(c)), t, &info()).unwrap();
        assert!(matches!(parse_reply(&theirs), Some(Reply::Challenge(_))));
        let ok = r.handle(from, &info_request(Some(c)), t, &info()).unwrap();
        assert_eq!(parse_reply(&ok), Some(Reply::Info(info())));
        // Still good in the next window; not two windows on.
        let later = t + Duration::from_secs(CHALLENGE_WINDOW);
        assert!(matches!(
            r.handle(from, &info_request(Some(c)), later, &info())
                .and_then(|b| parse_reply(&b)),
            Some(Reply::Info(_))
        ));
        let much_later = t + Duration::from_secs(3 * CHALLENGE_WINDOW);
        assert!(matches!(
            r.handle(from, &info_request(Some(c)), much_later, &info())
                .and_then(|b| parse_reply(&b)),
            Some(Reply::Challenge(_))
        ));
        assert!(
            r.handle(from, b"\xFF\xFF\xFF\xFFVxxxx", t, &info()).is_none(),
            "not a request"
        );
    }

    #[test]
    fn answers_are_rate_limited() {
        let mut r = Responder::new(7);
        let from: SocketAddr = "10.0.0.5:50000".parse().unwrap();
        let t = Duration::from_secs(5);
        let answered = (0..100)
            .filter(|_| r.handle(from, &info_request(None), t, &info()).is_some())
            .count();
        assert_eq!(answered, PER_ADDRESS.1 as usize, "a burst, then nothing");
        // A second later: the rate's worth more.
        let more = (0..100)
            .filter(|_| {
                r.handle(from, &info_request(None), t + Duration::from_secs(1), &info())
                    .is_some()
            })
            .count();
        assert_eq!(more, PER_ADDRESS.0 as usize);
        // In total, across many addresses.
        let mut r = Responder::new(7);
        let total = (0..2000u32)
            .filter(|i| {
                let a = SocketAddr::from((Ipv4Addr::from(0x0A00_0000 + i), 1000));
                r.handle(a, &info_request(None), t, &info()).is_some()
            })
            .count();
        assert_eq!(total, TOTAL.1 as usize);
    }
}
