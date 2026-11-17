//! The server browser's lists, as data (main-menu.md M2 and M3; no
//! drawing, no keys: `signal_pages` draws and drives them): the rows of
//! its four tabs (**Internet**: the servers the hub lists,
//! docs/plans/active/hub.md 4.3, each asked directly for its info and
//! ping, the hub's numbers filling a row until it answers; **LAN**: the
//! servers the LAN scan found, by ping; **Favourites**:
//! the ones the player kept, each asked for its info and ping by address,
//! the same query, unicast; **History**: the servers joined, newest
//! first, with when), the scrolling window over a long list, and the two
//! files they are kept in.
//!
//! - Internet (`internet_servers`): a listing of another app is left out
//!   (unless every game is asked for); the row is the server's own answer
//!   when the one at the listed address (or at one of its LAN addresses,
//!   for a player behind the same router) answers with the listing's
//!   instance id, the best ping of those; another `NET_VERSION` is greyed
//!   with "needs build ..."; a game the account may not play is greyed;
//!   the player's own server the hub can't reach shows the hub's hint.
//!   Sorted as Favourites are: answered by ping, then asked, then silent.
//!   `HubQueue` asks at most `MAX_QUERIES` servers at once.
//! - Files: `party_favourites.txt` (one address a line) and
//!   `party_history.txt` (`<unix seconds> <address>` a line, newest first,
//!   at most `HISTORY_MAX`) in the config folder, as `party_recent.txt` is
//!   (`BrowserFiles`; none under tests).
//! - Rows: `LanServer`, with a `Status` (up, still asked, no answer: a
//!   favourite or a visit that does not answer stays listed).
//! - Scrolling: `keep_in_view` and `scroll_by` move the first row shown
//!   so the focused row stays in a window of `VISIBLE_ROWS`.
//!
//! Known shortcuts: only addresses that are an IP (with or without a port,
//! 27015 by default) are kept, so a joined host name is in Recent only; a
//! row that answers as another game's server counts as not answering.

use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;

use bevy::prelude::*;
use chimera_net::net::hub::{Listing, Reach};
use chimera_net::net::query::{QueryResult, QueryState};

/// Rows of a list shown at once.
pub const VISIBLE_ROWS: usize = 8;
/// Favourites kept at most.
pub const FAVOURITES_MAX: usize = 64;
/// Joined servers kept at most.
pub const HISTORY_MAX: usize = 20;
/// The port of an address typed without one.
pub const DEFAULT_PORT: u16 = 27015;

/// Servers of the hub's list asked for their info at once, at most.
pub const MAX_QUERIES: usize = 64;

/// The browser's tabs (Internet first; LAN is the one shown without a
/// hub).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BrowserTab {
    Internet,
    #[default]
    Lan,
    Favourites,
    History,
}

impl BrowserTab {
    pub const ALL: [BrowserTab; 4] = [BrowserTab::Internet, BrowserTab::Lan, BrowserTab::Favourites, BrowserTab::History];

    pub fn name(self) -> &'static str {
        match self {
            BrowserTab::Internet => "Internet",
            BrowserTab::Lan => "LAN",
            BrowserTab::Favourites => "Favourites",
            BrowserTab::History => "History",
        }
    }

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|t| *t == self).unwrap_or(0)
    }

    /// Tab `i` (the last for a larger one).
    pub fn from_index(i: usize) -> Self {
        Self::ALL[i.min(Self::ALL.len() - 1)]
    }

    /// The tab `dir` steps along (stops at the ends).
    pub fn step(self, dir: i8) -> Self {
        let i = (self.index() as i32 + dir as i32).clamp(0, Self::ALL.len() as i32 - 1);
        Self::from_index(i as usize)
    }
}

/// Whether a server answered.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Status {
    Up,
    /// Asked, not answered yet.
    #[default]
    Asking,
    /// No answer (or not one of ours).
    Down,
}

/// A server of a list.
#[derive(Clone, Debug, PartialEq)]
pub struct LanServer {
    pub addr: SocketAddr,
    pub name: String,
    pub minigame: Option<String>,
    pub map: String,
    pub players: u8,
    pub max_players: u8,
    pub bots: u8,
    pub ping_ms: u32,
    pub locked: bool,
    /// Its network version is ours (else it can't be joined).
    pub same_version: bool,
    pub status: Status,
    /// When it was joined (unix seconds; the History tab).
    pub when: Option<u64>,
    /// It is one of the favourites.
    pub favourite: bool,
    /// Its numbers are the hub's (it hasn't answered itself yet).
    pub listed: bool,
    /// Why it can't be joined, or what its host should know ("needs
    /// build 0.1.0/net55", the hub's hint): shown under its name.
    pub note: Option<String>,
    /// It can't be joined for a reason other than its version (a game
    /// the account isn't invited to).
    pub barred: bool,
}

impl LanServer {
    /// A server that has not answered (yet): its address only.
    pub fn unanswered(addr: SocketAddr, status: Status) -> Self {
        LanServer {
            addr,
            name: addr.to_string(),
            minigame: None,
            map: String::new(),
            players: 0,
            max_players: 0,
            bots: 0,
            ping_ms: 0,
            locked: false,
            same_version: true,
            status,
            when: None,
            favourite: false,
            listed: false,
            note: None,
            barred: false,
        }
    }

    /// What it answered (None when it is not a server of ours).
    fn answered(r: &QueryResult) -> Option<LanServer> {
        match &r.state {
            QueryState::Answered { info, ping_ms } if info.is_ours() => Some(LanServer {
                addr: r.addr,
                name: if info.name.trim().is_empty() { r.addr.to_string() } else { info.name.clone() },
                minigame: info.minigame().map(str::to_string),
                map: info.map.clone(),
                players: info.players,
                max_players: info.max_players,
                bots: info.bots,
                ping_ms: *ping_ms,
                locked: info.password,
                same_version: info.version == chimera_net::net::NET_VERSION,
                status: Status::Up,
                when: None,
                favourite: false,
                listed: false,
                note: None,
                barred: false,
            }),
            _ => None,
        }
    }
}

fn by_ping(a: &LanServer, b: &LanServer) -> std::cmp::Ordering {
    a.ping_ms.cmp(&b.ping_ms).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
}

/// The LAN list from the queries' results: the servers found by a LAN
/// scan that answered, ours only, by ping (then name).
pub fn lan_servers(results: &[QueryResult]) -> Vec<LanServer> {
    let mut out: Vec<LanServer> = results.iter().filter(|r| r.lan).filter_map(LanServer::answered).collect();
    out.sort_by(by_ping);
    out
}

/// The rows for `addrs` (in that order) from the results: what each
/// answered, else its address with `Asking` (no result, or one pending)
/// or `Down`. `when` is each one's time (History).
pub fn listed(addrs: &[(SocketAddr, Option<u64>)], results: &[QueryResult]) -> Vec<LanServer> {
    addrs
        .iter()
        .map(|(addr, when)| {
            let r = results.iter().find(|r| r.addr == *addr);
            let mut row = match r {
                Some(r) => LanServer::answered(r).unwrap_or_else(|| {
                    let st = if matches!(r.state, QueryState::Pending) { Status::Asking } else { Status::Down };
                    LanServer::unanswered(*addr, st)
                }),
                None => LanServer::unanswered(*addr, Status::Asking),
            };
            row.when = *when;
            row
        })
        .collect()
}

/// A hub listing's instance id (16 hex digits) as the query's.
fn instance_of(listing: &Listing) -> Option<u64> {
    u64::from_str_radix(&listing.instance, 16).ok()
}

/// The addresses a listing may be reached at: the one the hub saw, then
/// its LAN ones.
pub fn listing_addrs(listing: &Listing) -> Vec<SocketAddr> {
    std::iter::once(&listing.addr).chain(&listing.lan).filter_map(|a| a.parse().ok()).collect()
}

/// The Internet tab's rows from the hub's `list` and the queries'
/// `results` (module doc). `game`: this app's id on the hub; `all_games`:
/// other apps' servers too (they can't be joined: greyed); `may_play`:
/// whether the account may play a game (hub.md 5.5).
pub fn internet_servers(list: &[Listing], results: &[QueryResult], game: &str, all_games: bool, may_play: &dyn Fn(&str) -> bool) -> Vec<LanServer> {
    let ours = chimera_net::net::NET_VERSION;
    let mut rows: Vec<LanServer> = list
        .iter()
        .filter(|l| all_games || l.game == game)
        .filter_map(|l| {
            let addrs = listing_addrs(l);
            let first = *addrs.first()?;
            let id = instance_of(l);
            // Its own answers: the listing's instance only (another server
            // may sit at a LAN address of the same number).
            let answer = addrs
                .iter()
                .filter_map(|a| results.iter().find(|r| r.addr == *a))
                .filter(|r| matches!(&r.state, QueryState::Answered { info, .. } if info.instance == id))
                .filter_map(LanServer::answered)
                .min_by(by_ping);
            let mut row = match answer {
                Some(row) => row,
                None => {
                    // The hub's numbers until it answers; silent when none
                    // of its addresses did.
                    let state = |a: &SocketAddr| results.iter().find(|r| r.addr == *a).map(|r| &r.state);
                    let silent = addrs.iter().all(|a| matches!(state(a), Some(QueryState::NoResponse | QueryState::Answered { .. })));
                    LanServer {
                        addr: first,
                        name: if l.name.trim().is_empty() { l.addr.clone() } else { l.name.clone() },
                        minigame: l.minigame.clone(),
                        map: l.map.clone(),
                        players: l.players,
                        max_players: l.max,
                        bots: l.bots,
                        ping_ms: 0,
                        locked: l.password,
                        same_version: l.net == ours,
                        status: if silent { Status::Down } else { Status::Asking },
                        when: None,
                        favourite: false,
                        listed: true,
                        note: None,
                        barred: false,
                    }
                }
            };
            if l.game != game {
                row.barred = true;
                row.note = Some(format!("another game: {}", l.game.replace('_', " ")));
            } else if !may_play(&l.game) {
                row.barred = true;
                row.note = Some("this account isn't invited to this game".into());
            } else if !row.same_version {
                row.note = Some(format!("needs build {}", l.net));
            } else if l.mine && l.reachable == Reach::No {
                row.note = l.hint.clone().or_else(|| Some("the hub can't reach it".into()));
            }
            Some(row)
        })
        .collect();
    sort_favourites(&mut rows);
    rows
}

/// The hub's list asked server by server, `MAX_QUERIES` at once: the
/// addresses waiting and the ones asked whose answer is awaited.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HubQueue {
    pub waiting: Vec<SocketAddr>,
    pub asked: Vec<SocketAddr>,
}

impl HubQueue {
    /// A new list: every address of it to ask (again).
    pub fn fill(&mut self, list: &[Listing]) {
        self.waiting.clear();
        self.asked.clear();
        for a in list.iter().flat_map(listing_addrs) {
            if !self.waiting.contains(&a) {
                self.waiting.push(a);
            }
        }
    }

    /// The addresses to ask now: `pending(addr)` says whether an asked
    /// one's answer is still awaited.
    pub fn next(&mut self, pending: impl Fn(SocketAddr) -> bool) -> Vec<SocketAddr> {
        self.asked.retain(|a| pending(*a));
        let room = MAX_QUERIES.saturating_sub(self.asked.len()).min(self.waiting.len());
        let now: Vec<SocketAddr> = self.waiting.drain(..room).collect();
        self.asked.extend(&now);
        now
    }

    pub fn busy(&self) -> bool {
        !self.waiting.is_empty() || !self.asked.is_empty()
    }
}

/// Favourites' order: the ones that answered by ping (then name), those
/// still asked, those that don't answer, then by address.
pub fn sort_favourites(rows: &mut [LanServer]) {
    let rank = |s: Status| match s {
        Status::Up => 0,
        Status::Asking => 1,
        Status::Down => 2,
    };
    rows.sort_by(|a, b| {
        rank(a.status)
            .cmp(&rank(b.status))
            .then_with(|| if a.status == Status::Up { by_ping(a, b) } else { a.addr.to_string().cmp(&b.addr.to_string()) })
    });
}

/// An address as typed: an IP with a port, or without (the default port).
/// None for a host name (not resolved here).
pub fn parse_addr(text: &str) -> Option<SocketAddr> {
    let t = text.trim();
    t.parse::<SocketAddr>().ok().or_else(|| t.parse::<IpAddr>().ok().map(|ip| SocketAddr::new(ip, DEFAULT_PORT)))
}

/// A joined server and when (unix seconds).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Visit {
    pub addr: SocketAddr,
    pub when: u64,
}

/// `addr` first in the favourites' list if it was not in it, else removed
/// from it. True when it was added.
pub fn toggle_favourite(list: &mut Vec<SocketAddr>, addr: SocketAddr) -> bool {
    if let Some(i) = list.iter().position(|a| *a == addr) {
        list.remove(i);
        false
    } else {
        list.push(addr);
        list.truncate(FAVOURITES_MAX);
        true
    }
}

/// A join of `addr` at `when`: newest first, one entry for a server.
pub fn record_visit(list: &mut Vec<Visit>, addr: SocketAddr, when: u64) {
    list.retain(|v| v.addr != addr);
    list.insert(0, Visit { addr, when });
    list.truncate(HISTORY_MAX);
}

/// Now, unix seconds.
pub fn unix_now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// How long ago `then` was at `now`: "just now", "5 min ago", "3 h ago",
/// "2 days ago".
pub fn ago(now: u64, then: u64) -> String {
    let s = now.saturating_sub(then);
    match s {
        0..=59 => "just now".into(),
        60..=3599 => format!("{} min ago", s / 60),
        3600..=86_399 => format!("{} h ago", s / 3600),
        _ => {
            let d = s / 86_400;
            if d == 1 { "1 day ago".into() } else { format!("{d} days ago") }
        }
    }
}

/// The first row shown so `focus` (a row's index) is in the window of
/// `rows`, moving `offset` as little as can be.
pub fn keep_in_view(offset: usize, focus: usize, len: usize, rows: usize) -> usize {
    let rows = rows.max(1);
    let o = if focus < offset {
        focus
    } else if focus >= offset + rows {
        focus + 1 - rows
    } else {
        offset
    };
    clamp_offset(o, len, rows)
}

/// `offset` within what a list of `len` rows can scroll to.
pub fn clamp_offset(offset: usize, len: usize, rows: usize) -> usize {
    offset.min(len.saturating_sub(rows.max(1)))
}

/// `offset` moved `delta` rows (a wheel notch).
pub fn scroll_by(offset: usize, delta: i32, len: usize, rows: usize) -> usize {
    clamp_offset((offset as i64 + delta as i64).clamp(0, len as i64) as usize, len, rows)
}

/// Where the favourites and the history are kept (None: not kept; tests).
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct BrowserFiles {
    pub favourites: Option<PathBuf>,
    pub history: Option<PathBuf>,
}

impl BrowserFiles {
    /// `party_favourites.txt` and `party_history.txt` in the config folder.
    pub fn in_config() -> Self {
        let dir = chimera_core::console::cfg_dir();
        Self {
            favourites: dir.as_ref().map(|d| d.join("party_favourites.txt")),
            history: dir.map(|d| d.join("party_history.txt")),
        }
    }

    pub fn read_favourites(&self) -> Vec<SocketAddr> {
        self.favourites.as_ref().and_then(|p| std::fs::read_to_string(p).ok()).map(|t| parse_favourites(&t)).unwrap_or_default()
    }

    pub fn read_history(&self) -> Vec<Visit> {
        self.history.as_ref().and_then(|p| std::fs::read_to_string(p).ok()).map(|t| parse_history(&t)).unwrap_or_default()
    }

    pub fn write_favourites(&self, list: &[SocketAddr]) {
        write(&self.favourites, list.iter().map(|a| a.to_string()).collect::<Vec<_>>().join("\n"));
    }

    pub fn write_history(&self, list: &[Visit]) {
        write(&self.history, list.iter().map(|v| format!("{} {}", v.when, v.addr)).collect::<Vec<_>>().join("\n"));
    }
}

fn write(path: &Option<PathBuf>, text: String) {
    let Some(p) = path else { return };
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Err(e) = std::fs::write(p, text + "\n") {
        warn!("server browser: writing {}: {e}", p.display());
    }
}

/// The favourites file's text: an address a line (bad lines and repeats
/// dropped).
pub fn parse_favourites(text: &str) -> Vec<SocketAddr> {
    let mut out: Vec<SocketAddr> = Vec::new();
    for a in text.lines().filter_map(parse_addr) {
        if !out.contains(&a) {
            out.push(a);
        }
    }
    out.truncate(FAVOURITES_MAX);
    out
}

/// The history file's text: `<unix seconds> <address>` a line.
pub fn parse_history(text: &str) -> Vec<Visit> {
    let mut out: Vec<Visit> = Vec::new();
    for l in text.lines() {
        let Some((when, addr)) = l.trim().split_once(char::is_whitespace) else { continue };
        let (Ok(when), Some(addr)) = (when.parse::<u64>(), parse_addr(addr)) else { continue };
        if !out.iter().any(|v| v.addr == addr) {
            out.push(Visit { addr, when });
        }
    }
    out.truncate(HISTORY_MAX);
    out
}
