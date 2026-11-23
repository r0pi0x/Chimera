//! The hub's session-only parties (docs/plans/active/parties.md): the
//! model, the limits and the `parties` table; the routes are
//! `http`'s `POST /hub/party/<what>`, the JSON `chimera_assets::party_doc`.
//!
//! - **A party**: a public id (12 hex digits), an invite code (6
//!   Crockford characters, `codes::generate`, kept normalized, unique
//!   among live parties), a leader, at most `party_doc::MAX_MEMBERS`
//!   members in join order, the leader's target and its move counter.
//! - **A member**: its id (8 hex digits), name (20 characters, no control
//!   characters) and avatar as its last call said, its account's username
//!   and display name as that call's ticket says (a new display name
//!   shows at the member's next ticket), the SHA-256 of its
//!   member token (the token itself is answered once, at `create` or
//!   `join`), its account (for the later step that ties parties to
//!   accounts), when it was last seen (any call; in memory only: a
//!   restart counts as seen) and the server it is on.
//! - **Leadership**: the leader's `kick`, `promote` and `lead` (403 for
//!   anyone else); a leader who leaves or is dropped hands it to the
//!   member who joined first, online ones first; a leader offline for
//!   `Options::lead_handover` while another is online hands it on too.
//! - **Expiry** (`sweep_parties`, the hub's sweep): a member unseen for
//!   `Options::member_expiry` is dropped; an empty party goes, its code
//!   free again.
//! - **Limits**: `Options::party_calls` calls a minute per account (429);
//!   wrong codes per account and per address at most
//!   `Options::code_failures` in `CODE_WINDOW` s, then 429 until the
//!   oldest falls out.
//! - **Persistence**: one JSON row per party in `hub.db`'s `parties`,
//!   written on each change (not on a poll that changes nothing but the
//!   time seen).
//!
//! Known shortcuts: an account may hold any number of memberships (the
//! client leaves its party before it creates or joins another); the
//! limits are in memory (a restart forgets them).

use std::{
    collections::{HashMap, VecDeque},
    net::{IpAddr, SocketAddr},
};

use chimera_assets::{
    party_doc::{
        Create, Join, Joined, Lead, MAX_MEMBERS, MemberView, MoveKind, ONLINE_SECS, OnMember, PartyView, State,
        TOKEN_PREFIX, Target,
    },
    store::hex,
    ticket::Claims,
};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{Hub, internal, refuse};
use crate::{accounts::R, codes};

/// An invite code's characters.
pub const CODE_LEN: usize = 6;
/// The window wrong codes are counted in, s.
pub const CODE_WINDOW: u64 = 600;
/// A name's longest, in characters.
pub const NAME_MAX: usize = 20;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Member {
    id: String,
    /// SHA-256 (hex) of the member token.
    token: String,
    account: String,
    name: String,
    /// The account's username and display name, from its last call's
    /// ticket (rows saved before them: empty).
    #[serde(default)]
    username: String,
    #[serde(default)]
    display_name: String,
    avatar: u8,
    joined: u64,
    #[serde(default)]
    at: Option<String>,
    #[serde(skip)]
    seen: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct Party {
    id: String,
    /// Normalized (no dash).
    code: String,
    leader: String,
    members: Vec<Member>,
    #[serde(default)]
    target: Option<Target>,
    #[serde(default)]
    moves: u32,
    #[serde(default)]
    kind: MoveKind,
    #[serde(default)]
    left: Option<String>,
}

impl Party {
    fn shown_code(&self) -> String {
        format!("{}-{}", &self.code[..3], &self.code[3..])
    }

    fn view(&self, you: &str, now: u64) -> PartyView {
        PartyView {
            id: self.id.clone(),
            code: self.shown_code(),
            members: self
                .members
                .iter()
                .map(|m| MemberView {
                    id: m.id.clone(),
                    name: m.name.clone(),
                    username: m.username.clone(),
                    display_name: m.display_name.clone(),
                    avatar: m.avatar,
                    online: now < m.seen + ONLINE_SECS,
                    leader: m.id == self.leader,
                    at: m.at.clone(),
                })
                .collect(),
            you: you.to_string(),
            target: self.target.clone(),
            moves: self.moves,
            kind: self.kind,
            left: self.left.clone(),
        }
    }

    /// The lead to the member who joined first, online ones first.
    fn pass_lead(&mut self, now: u64) {
        let online = |m: &&Member| now < m.seen + ONLINE_SECS;
        if let Some(m) = self.members.iter().find(online).or(self.members.first()) {
            self.leader = m.id.clone();
        }
    }
}

/// The parties in memory and the limits' counts.
#[derive(Default)]
pub(super) struct Book {
    parties: HashMap<String, Party>,
    /// Token hash -> (party, member).
    tokens: HashMap<String, (String, String)>,
    /// Normalized code -> party.
    codes: HashMap<String, String>,
    /// Calls per account (the minute's times).
    calls: HashMap<String, VecDeque<u64>>,
    /// Wrong codes per account and per address.
    failures: HashMap<String, VecDeque<u64>>,
}

impl Book {
    fn insert(&mut self, p: Party) {
        for m in &p.members {
            self.tokens.insert(m.token.clone(), (p.id.clone(), m.id.clone()));
        }
        self.codes.insert(p.code.clone(), p.id.clone());
        self.parties.insert(p.id.clone(), p);
    }

    fn remove(&mut self, id: &str) {
        if let Some(p) = self.parties.remove(id) {
            for m in &p.members {
                self.tokens.remove(&m.token);
            }
            self.codes.remove(&p.code);
        }
    }
}

fn random_hex(bytes: usize) -> String {
    let mut b = vec![0u8; bytes];
    getrandom::fill(&mut b).expect("the OS's randomness");
    hex(&b)
}

fn hash(token: &str) -> String {
    hex(&Sha256::digest(token.trim().as_bytes()))
}

fn clean_name(name: &str) -> R<String> {
    if name.chars().any(char::is_control) {
        return Err(refuse(400, "name: no control characters"));
    }
    let n: String = name.trim().chars().take(NAME_MAX).collect();
    Ok(if n.is_empty() { "Player".into() } else { n })
}

fn clean_at(at: Option<&str>) -> R<Option<String>> {
    match at.map(str::trim).filter(|a| !a.is_empty()) {
        None => Ok(None),
        Some(a) => a.parse::<SocketAddr>().map(|s| Some(s.to_string())).map_err(|_| refuse(400, "at: a server's ip:port")),
    }
}

fn not_member() -> crate::accounts::Refusal {
    refuse(404, "not in a party")
}

impl Hub {
    fn book(&self) -> std::sync::MutexGuard<'_, Book> {
        self.parties.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The parties kept in `hub.db`, each member seen now.
    pub(super) fn load_parties(&self) -> Result<(), String> {
        let now = self.now();
        let conn = self.db();
        let mut stmt = conn.prepare("SELECT doc FROM parties").map_err(|e| e.to_string())?;
        let docs: Vec<String> = stmt.query_map([], |r| r.get::<_, String>(0)).map_err(|e| e.to_string())?.filter_map(Result::ok).collect();
        let mut book = self.book();
        for doc in docs {
            if let Ok(mut p) = serde_json::from_str::<Party>(&doc) {
                for m in &mut p.members {
                    m.seen = now;
                }
                book.insert(p);
            }
        }
        Ok(())
    }

    fn save_party(&self, p: &Party) -> R<()> {
        let doc = serde_json::to_string(p).map_err(internal)?;
        self.db().execute("INSERT OR REPLACE INTO parties (id, doc) VALUES (?1, ?2)", params![p.id, doc]).map(drop).map_err(internal)
    }

    fn drop_party(&self, id: &str) {
        if let Err(e) = self.db().execute("DELETE FROM parties WHERE id = ?1", params![id]) {
            eprintln!("hub: {e}");
        }
    }

    /// A call by `claims`' account: counted, 429 past the minute's limit.
    fn count_call(&self, book: &mut Book, claims: &Claims) -> R<()> {
        let now = self.now();
        let q = book.calls.entry(hex(&claims.account)).or_default();
        while q.front().is_some_and(|t| *t + 60 <= now) {
            q.pop_front();
        }
        if q.len() >= self.options.party_calls {
            return Err(refuse(429, format!("at most {} party calls a minute", self.options.party_calls)));
        }
        q.push_back(now);
        Ok(())
    }

    fn failure_keys(claims: &Claims, ip: Option<IpAddr>) -> Vec<String> {
        let mut k = vec![format!("a:{}", hex(&claims.account))];
        if let Some(ip) = ip {
            k.push(format!("i:{ip}"));
        }
        k
    }

    fn too_many_failures(&self, book: &mut Book, keys: &[String]) -> bool {
        let now = self.now();
        keys.iter().any(|k| {
            let q = book.failures.entry(k.clone()).or_default();
            while q.front().is_some_and(|t| *t + CODE_WINDOW <= now) {
                q.pop_front();
            }
            q.len() >= self.options.code_failures
        })
    }

    fn new_member(&self, claims: &Claims, name: &str, avatar: u8, taken: &[Member]) -> R<(String, Member)> {
        let token = format!("{TOKEN_PREFIX}{}", random_hex(16));
        let id = loop {
            let id = random_hex(4);
            if !taken.iter().any(|m| m.id == id) {
                break id;
            }
        };
        let now = self.now();
        Ok((
            token.clone(),
            Member {
                id,
                token: hash(&token),
                account: hex(&claims.account),
                name: clean_name(name)?,
                username: claims.username.clone(),
                display_name: claims.display_name.clone(),
                avatar,
                joined: now,
                at: None,
                seen: now,
            },
        ))
    }

    /// `POST /hub/party/create`: a new party with the asker its leader.
    pub fn party_create(&self, claims: &Claims, body: &Create) -> R<Joined> {
        let mut book = self.book();
        self.count_call(&mut book, claims)?;
        let (token, me) = self.new_member(claims, &body.me.name, body.me.avatar, &[])?;
        let id = loop {
            let id = random_hex(6);
            if !book.parties.contains_key(&id) {
                break id;
            }
        };
        let code = loop {
            let c = codes::normalize(&codes::generate(CODE_LEN)).expect("a generated code reads back");
            if !book.codes.contains_key(&c) {
                break c;
            }
        };
        let party = Party {
            id,
            code,
            leader: me.id.clone(),
            members: vec![me.clone()],
            target: None,
            moves: 0,
            kind: MoveKind::None,
            left: None,
        };
        self.save_party(&party)?;
        let view = party.view(&me.id, self.now());
        book.insert(party);
        Ok(Joined { token, party: view })
    }

    /// `POST /hub/party/join`: into the party whose code it is.
    pub fn party_join(&self, claims: &Claims, body: &Join, ip: Option<IpAddr>) -> R<Joined> {
        let mut book = self.book();
        self.count_call(&mut book, claims)?;
        let keys = Self::failure_keys(claims, ip);
        if self.too_many_failures(&mut book, &keys) {
            return Err(refuse(429, "too many wrong party codes: try again in a few minutes"));
        }
        let found = codes::normalize(&body.code).filter(|c| c.len() == CODE_LEN).and_then(|c| book.codes.get(&c).cloned());
        let Some(id) = found else {
            let now = self.now();
            for k in keys {
                book.failures.entry(k).or_default().push_back(now);
            }
            return Err(refuse(404, "no party has that code"));
        };
        let party = book.parties.get(&id).expect("a code names a live party");
        if party.members.len() >= MAX_MEMBERS {
            return Err(refuse(409, format!("that party is full ({MAX_MEMBERS})")));
        }
        let (token, me) = self.new_member(claims, &body.me.name, body.me.avatar, &party.members)?;
        let party = book.parties.get_mut(&id).expect("live");
        party.members.push(me.clone());
        let p = party.clone();
        book.tokens.insert(me.token.clone(), (id, me.id.clone()));
        self.save_party(&p)?;
        Ok(Joined { token, party: p.view(&me.id, self.now()) })
    }

    /// The party and member `token` is, counted as a call; 404 if none.
    fn member<'a>(&self, book: &'a mut Book, claims: &Claims, token: &str) -> R<(&'a mut Party, String)> {
        self.count_call(book, claims)?;
        let (pid, mid) = book.tokens.get(&hash(token)).cloned().ok_or_else(not_member)?;
        let now = self.now();
        let party = book.parties.get_mut(&pid).ok_or_else(not_member)?;
        if let Some(m) = party.members.iter_mut().find(|m| m.id == mid) {
            m.seen = now;
        }
        Ok((party, mid))
    }

    /// `POST /hub/party/state`: the heartbeat; the party.
    pub fn party_state(&self, claims: &Claims, body: &State) -> R<PartyView> {
        let at = clean_at(body.at.as_deref())?;
        let name = clean_name(&body.me.name)?;
        let now = self.now();
        let mut book = self.book();
        let (party, me) = self.member(&mut book, claims, &body.token)?;
        let m = party.members.iter_mut().find(|m| m.id == me).expect("a member");
        let changed = m.at != at
            || m.name != name
            || m.avatar != body.me.avatar
            || m.username != claims.username
            || m.display_name != claims.display_name;
        (m.at, m.name, m.avatar) = (at, name, body.me.avatar);
        (m.username, m.display_name) = (claims.username.clone(), claims.display_name.clone());
        let p = party.clone();
        if changed {
            self.save_party(&p)?;
        }
        Ok(p.view(&me, now))
    }

    /// `POST /hub/party/leave`.
    pub fn party_leave(&self, claims: &Claims, token: &str) -> R<()> {
        let now = self.now();
        let mut book = self.book();
        let (party, me) = self.member(&mut book, claims, token)?;
        let pid = party.id.clone();
        party.members.retain(|m| m.id != me);
        if party.members.is_empty() {
            book.remove(&pid);
            drop(book);
            self.drop_party(&pid);
            return Ok(());
        }
        if party.leader == me {
            party.pass_lead(now);
        }
        let p = party.clone();
        book.tokens.remove(&hash(token));
        self.save_party(&p)
    }

    fn leader_only<'a>(&self, book: &'a mut Book, claims: &Claims, token: &str) -> R<(&'a mut Party, String)> {
        let (party, me) = self.member(book, claims, token)?;
        if party.leader != me {
            return Err(refuse(403, "only the party's leader may"));
        }
        Ok((party, me))
    }

    /// `POST /hub/party/kick`: the leader puts `member` out.
    pub fn party_kick(&self, claims: &Claims, body: &OnMember) -> R<PartyView> {
        let now = self.now();
        let mut book = self.book();
        let (party, me) = self.leader_only(&mut book, claims, &body.token)?;
        if body.member == me {
            return Err(refuse(400, "to leave, use Leave"));
        }
        let gone = party.members.iter().position(|m| m.id == body.member).ok_or_else(|| refuse(400, "no such member"))?;
        let gone = party.members.remove(gone);
        let p = party.clone();
        book.tokens.remove(&gone.token);
        self.save_party(&p)?;
        Ok(p.view(&me, now))
    }

    /// `POST /hub/party/promote`: the leader hands the lead to `member`.
    pub fn party_promote(&self, claims: &Claims, body: &OnMember) -> R<PartyView> {
        let now = self.now();
        let mut book = self.book();
        let (party, me) = self.leader_only(&mut book, claims, &body.token)?;
        if !party.members.iter().any(|m| m.id == body.member) {
            return Err(refuse(400, "no such member"));
        }
        party.leader = body.member.clone();
        let p = party.clone();
        self.save_party(&p)?;
        Ok(p.view(&me, now))
    }

    /// `POST /hub/party/lead`: the leader takes the party to a server, or
    /// out of one (with it, or alone).
    pub fn party_lead(&self, claims: &Claims, body: &Lead, peer: Option<IpAddr>) -> R<PartyView> {
        let target = match &body.target {
            None => None,
            Some(t) => {
                let addr = if body.hosted {
                    let port: u16 = t.addr.rsplit(':').next().and_then(|p| p.parse().ok()).filter(|p| *p != 0).ok_or_else(|| refuse(400, "target: a port"))?;
                    let ip = peer.ok_or_else(|| refuse(400, "no address"))?;
                    let ip = if ip.is_loopback() {
                        self.config().public_address.and_then(|a| a.parse().ok()).unwrap_or(ip)
                    } else {
                        ip
                    };
                    SocketAddr::new(ip, port).to_string()
                } else {
                    clean_at(Some(&t.addr))?.ok_or_else(|| refuse(400, "target: a server's ip:port"))?
                };
                Some(Target { addr, name: clean_name(&t.name).map(|n| if t.name.trim().is_empty() { String::new() } else { n })? })
            }
        };
        let now = self.now();
        let mut book = self.book();
        let (party, me) = self.leader_only(&mut book, claims, &body.token)?;
        match (target, body.together) {
            (Some(t), _) => {
                if party.target.as_ref().map(|x| &x.addr) != Some(&t.addr) || party.kind != MoveKind::Join {
                    party.moves += 1;
                    party.kind = MoveKind::Join;
                    party.left = None;
                }
                party.target = Some(t);
            }
            (None, true) => {
                party.left = party.target.take().map(|t| t.addr);
                party.moves += 1;
                party.kind = MoveKind::Leave;
            }
            (None, false) => {
                party.target = None;
            }
        }
        let p = party.clone();
        self.save_party(&p)?;
        Ok(p.view(&me, now))
    }

    /// Members unseen too long dropped, empty parties gone, an offline
    /// leader's lead handed on (the hub's sweep).
    pub(super) fn sweep_parties(&self) {
        let now = self.now();
        let (expiry, handover) = (self.options.member_expiry, self.options.lead_handover);
        let mut book = self.book();
        let mut changed = Vec::new();
        let mut empty = Vec::new();
        let mut gone_tokens = Vec::new();
        for p in book.parties.values_mut() {
            let before = (p.members.len(), p.leader.clone());
            p.members.retain(|m| {
                let keep = now < m.seen + expiry;
                if !keep {
                    gone_tokens.push(m.token.clone());
                }
                keep
            });
            if p.members.is_empty() {
                empty.push(p.id.clone());
                continue;
            }
            let leader_seen = p.members.iter().find(|m| m.id == p.leader).map(|m| m.seen);
            let other_online = p.members.iter().any(|m| m.id != p.leader && now < m.seen + ONLINE_SECS);
            if leader_seen.is_none() || (leader_seen.is_some_and(|s| now >= s + handover) && other_online) {
                p.pass_lead(now);
            }
            if before != (p.members.len(), p.leader.clone()) {
                changed.push(p.clone());
            }
        }
        for t in gone_tokens {
            book.tokens.remove(&t);
        }
        for id in &empty {
            book.remove(id);
        }
        drop(book);
        for id in &empty {
            self.drop_party(id);
        }
        for p in &changed {
            let _ = self.save_party(p);
        }
    }

    /// Live parties (`/health`).
    pub fn party_count(&self) -> usize {
        self.book().parties.len()
    }
}
