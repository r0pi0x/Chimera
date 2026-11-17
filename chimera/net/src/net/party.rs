//! The player's party from a game process (docs/plans/active/parties.md):
//! the hub's party calls (`chimera_assets::party_doc`) on threads, the
//! member token kept in `party.toml`, the party id told to game servers
//! (`cl_party`), the leader's moves told to the hub, and the members
//! following them.
//!
//! - **`PartyClient`** (a resource): the hub seam (`PartyHub`: the real
//!   one is `HttpPartyHub`, the account's hub over HTTPS; tests put a fake
//!   in memory), the token and the party as last answered (`view`), why
//!   the last call failed, a follow prompt and a note for the screens.
//!   Apps start it from the user's hub (`use_user_party`, after
//!   `hub::use_user_hub`); a token in `party.toml` for that hub is used at
//!   once, so reopening the game keeps the player in its party.
//! - **The heartbeat**: the party's state asked every `POLL_SECS` (real
//!   time) and right after any action, with where this process is (`at`:
//!   the server a client has joined). A 404 forgets the token (left,
//!   kicked or expired).
//! - **`cl_party`** (userinfo, not saved): the party's public id, empty
//!   alone; the server groups and teams by it (`prematch::PartyNumbers`,
//!   `server::spawn_player`).
//! - **The leader's moves**: a leader that joined a server (a client, its
//!   map in) or hosts one (a listen server: `hosted`, the hub fills in
//!   its address) tells the hub (`lead`) once per place.
//!   `party_leave_match with|alone` (the leader's Leave dialog) tells it
//!   "out, together" or "out, alone", then disconnects.
//! - **Following** (`follow`, a pure rule): a member acts once on each
//!   move: *join* on the menu goes there at once (`PartyGo::Join`, acted on by `act`),
//!   in another match asks (`prompt`: `party_follow` joins, `party_stay`
//!   dismisses); *leave* on the server the party left goes back to the
//!   menu (`PartyGo::Leave`) with a note. The first answer after a
//!   start or a join acts on no old move; it asks when the party is
//!   somewhere else.
//! - Console: `party`, `party_create`, `party_join <code>`,
//!   `party_leave`, `party_kick <member id|name>`, `party_promote <member
//!   id|name>`, `party_follow`, `party_stay`, `party_leave_match
//!   [with|alone]`.
//!
//! Known shortcuts: the poll timer reads `Time<Real>` (tests call
//! `PartyClient::poll_now`); a leader who disconnects other than through
//! the Leave dialog leaves the party's target where it was (members stay,
//! a later join there is no new move).

use std::{
    net::SocketAddr,
    path::PathBuf,
    sync::{Arc, Mutex, mpsc},
};

use bevy::prelude::*;
use chimera_assets::party_doc::{self, Create, Failure, Join, Joined, Lead, OnMember, POLL_SECS, PartyFile, State};
/// The hub's party JSON and calls (`chimera_assets::party_doc`).
pub use chimera_assets::party_doc as doc;
pub use chimera_assets::party_doc::{MAX_MEMBERS, Me, MemberView, MoveKind, PartyView, Target};

use crate::{
    console::{Console, ConsoleAppExt, resource_cvar},
    core::NetRole,
};

/// The hub's party calls (blocking; `PartyClient` runs them on threads).
pub trait PartyHub: Send + Sync + 'static {
    /// The hub's origin (which hub a token in `party.toml` belongs to).
    fn origin(&self) -> String;
    fn create(&self, body: &Create) -> Result<Joined, Failure>;
    fn join(&self, body: &Join) -> Result<Joined, Failure>;
    fn state(&self, body: &State) -> Result<PartyView, Failure>;
    fn leave(&self, token: &str) -> Result<(), Failure>;
    fn kick(&self, body: &OnMember) -> Result<PartyView, Failure>;
    fn promote(&self, body: &OnMember) -> Result<PartyView, Failure>;
    fn lead(&self, body: &Lead) -> Result<PartyView, Failure>;
}

/// The account's hub over HTTPS, with that account's tickets.
pub struct HttpPartyHub {
    pub origin: String,
    pub fingerprint: Option<String>,
    /// The account file (None: the user's).
    pub account: Option<PathBuf>,
}

impl HttpPartyHub {
    fn ticket(&self) -> Result<String, Failure> {
        let fail = |e: String| Failure { status: None, message: e };
        super::hub::session_for(self.account.as_deref()).map_err(fail)?.ticket().map_err(fail)
    }

    fn fp(&self) -> Option<&str> {
        self.fingerprint.as_deref()
    }
}

impl PartyHub for HttpPartyHub {
    fn origin(&self) -> String {
        self.origin.clone()
    }
    fn create(&self, body: &Create) -> Result<Joined, Failure> {
        party_doc::create(&self.origin, self.fp(), &self.ticket()?, body)
    }
    fn join(&self, body: &Join) -> Result<Joined, Failure> {
        party_doc::join(&self.origin, self.fp(), &self.ticket()?, body)
    }
    fn state(&self, body: &State) -> Result<PartyView, Failure> {
        party_doc::state(&self.origin, self.fp(), &self.ticket()?, body)
    }
    fn leave(&self, token: &str) -> Result<(), Failure> {
        party_doc::leave(&self.origin, self.fp(), &self.ticket()?, token)
    }
    fn kick(&self, body: &OnMember) -> Result<PartyView, Failure> {
        party_doc::kick(&self.origin, self.fp(), &self.ticket()?, body)
    }
    fn promote(&self, body: &OnMember) -> Result<PartyView, Failure> {
        party_doc::promote(&self.origin, self.fp(), &self.ticket()?, body)
    }
    fn lead(&self, body: &Lead) -> Result<PartyView, Failure> {
        party_doc::lead(&self.origin, self.fp(), &self.ticket()?, body)
    }
}

/// What a call came back with.
enum Answer {
    /// A new membership (create, join): the token and the party.
    Joined(Result<Joined, Failure>),
    /// The party (state, kick, promote, lead), for the token asked with.
    View(String, Result<PartyView, Failure>),
    /// Left (or tried to).
    Left(Result<(), Failure>),
}

/// The player's party (module doc).
#[derive(Resource, Default)]
pub struct PartyClient {
    pub hub: Option<Arc<dyn PartyHub>>,
    /// `party.toml` (None: not kept; tests).
    pub file: Option<PathBuf>,
    pub token: Option<String>,
    /// The party as last answered.
    pub view: Option<PartyView>,
    /// Why the last call failed.
    pub error: Option<String>,
    /// A move to ask about: "Your party went to <name>: Join / Stay".
    pub prompt: Option<Target>,
    /// A line for the screens ("Your party's leader left").
    pub note: Option<String>,
    /// Calls under way.
    pub busy: usize,
    /// Bumped at each answer (screens rebuild).
    pub generation: u64,
    /// The last move acted on (None: no answer yet since the token came).
    handled: Option<u32>,
    /// The place last told the hub as the leader.
    told: Option<String>,
    /// When the next poll is due (`Time<Real>` s).
    next_poll: f64,
    pending: Mutex<Vec<mpsc::Receiver<Answer>>>,
}

impl PartyClient {
    /// A client of `hub`, the token kept at `file` (one there for this hub
    /// is taken up).
    pub fn new(hub: Arc<dyn PartyHub>, file: Option<PathBuf>) -> Self {
        let token = file.as_deref().and_then(PartyFile::load_from).filter(|f| f.hub == hub.origin()).map(|f| f.token);
        Self { hub: Some(hub), file, token, ..default() }
    }

    pub fn available(&self) -> bool {
        self.hub.is_some()
    }

    /// In a party (as last answered).
    pub fn in_party(&self) -> bool {
        self.token.is_some() && self.view.is_some()
    }

    /// The asker leads a party of two or more.
    pub fn leads_others(&self) -> bool {
        self.view.as_ref().is_some_and(|v| v.leading() && v.members.len() > 1)
    }

    fn spawn(&mut self, job: impl FnOnce(&dyn PartyHub) -> Answer + Send + 'static) {
        let Some(hub) = self.hub.clone() else { return };
        let (tx, rx) = mpsc::channel();
        let busy = chimera_core::tasks::Busy::new();
        std::thread::Builder::new()
            .name("party".into())
            .spawn(move || {
                let _busy = busy;
                let _ = tx.send(job(hub.as_ref()));
            })
            .expect("a thread");
        self.busy += 1;
        self.pending.lock().unwrap_or_else(|e| e.into_inner()).push(rx);
    }

    /// Ask the party's state at the next frame.
    pub fn poll_now(&mut self) {
        self.next_poll = f64::MIN;
    }

    /// A new party, led by us (leaving the one we're in first).
    pub fn create(&mut self, me: Me) {
        let old = self.token.clone();
        self.spawn(move |h| {
            if let Some(t) = old {
                let _ = h.leave(&t);
            }
            Answer::Joined(h.create(&Create { me }))
        });
    }

    /// Into the party whose invite code is `code` (leaving ours first).
    pub fn join(&mut self, code: &str, me: Me) {
        let (old, code) = (self.token.clone(), code.to_string());
        self.spawn(move |h| {
            if let Some(t) = old {
                let _ = h.leave(&t);
            }
            Answer::Joined(h.join(&Join { code, me }))
        });
    }

    /// Out of the party (forgotten at once; the hub told on a thread).
    pub fn leave(&mut self) {
        let Some(t) = self.token.take() else { return };
        self.forget();
        self.spawn(move |h| Answer::Left(h.leave(&t)));
    }

    pub fn kick(&mut self, member: &str) {
        self.on_member(member, |h, b| h.kick(b));
    }

    pub fn promote(&mut self, member: &str) {
        self.on_member(member, |h, b| h.promote(b));
    }

    fn on_member(&mut self, member: &str, call: fn(&dyn PartyHub, &OnMember) -> Result<PartyView, Failure>) {
        let Some(token) = self.token.clone() else { return };
        let member = member.to_string();
        self.spawn(move |h| Answer::View(token.clone(), call(h, &OnMember { token, member })));
    }

    /// The leader's move (`target` none: out, `together` or alone).
    pub fn lead(&mut self, target: Option<Target>, together: bool, hosted: bool) {
        let Some(token) = self.token.clone() else { return };
        // Out of a server: `told` stays the place until this process has
        // left it, so the frames before the disconnect don't lead back in.
        if let Some(t) = &target {
            self.told = Some(t.addr.clone());
        }
        self.spawn(move |h| Answer::View(token.clone(), h.lead(&Lead { token, target, together, hosted })));
    }

    fn state(&mut self, me: Me, at: Option<String>) {
        let Some(token) = self.token.clone() else { return };
        self.spawn(move |h| Answer::View(token.clone(), h.state(&State { token, me, at })));
    }

    fn forget(&mut self) {
        self.view = None;
        self.prompt = None;
        self.handled = None;
        self.told = None;
        if let Some(f) = &self.file {
            PartyFile::remove(f);
        }
        self.generation += 1;
    }

    fn keep(&self) {
        if let (Some(f), Some(t), Some(h)) = (&self.file, &self.token, &self.hub)
            && let Err(e) = (PartyFile { hub: h.origin(), token: t.clone() }).save_to(f)
        {
            warn!("party: keeping its token: {e}");
        }
    }

    fn take_answers(&mut self) -> Vec<Answer> {
        let mut p = self.pending.lock().unwrap_or_else(|e| e.into_inner());
        let mut out = Vec::new();
        p.retain(|rx| match rx.try_recv() {
            Ok(a) => {
                out.push(a);
                false
            }
            Err(mpsc::TryRecvError::Empty) => true,
            Err(mpsc::TryRecvError::Disconnected) => false,
        });
        out
    }
}

/// What a member does about the party's latest move (module doc).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Follow {
    Nothing,
    /// Go there now (on the menu).
    Join(Target),
    /// Ask first (in another match, or the party was there already).
    Ask(Target),
    /// Back to the menu (the party left this server).
    Leave,
}

/// The rule: `handled` the last move acted on (None: the first answer),
/// `at` the server this process is on, `hosting` it serves a game.
pub fn follow(view: &PartyView, handled: Option<u32>, at: Option<&str>, hosting: bool) -> Follow {
    if view.leading() {
        return Follow::Nothing;
    }
    let elsewhere = |t: &Target| at != Some(t.addr.as_str());
    let Some(handled) = handled else {
        return match &view.target {
            Some(t) if view.kind == MoveKind::Join && elsewhere(t) => Follow::Ask(t.clone()),
            _ => Follow::Nothing,
        };
    };
    if view.moves <= handled {
        return Follow::Nothing;
    }
    match (view.kind, &view.target) {
        (MoveKind::Join, Some(t)) if elsewhere(t) => {
            if at.is_none() && !hosting { Follow::Join(t.clone()) } else { Follow::Ask(t.clone()) }
        }
        (MoveKind::Leave, _) if at.is_some() && at == view.left.as_deref() => Follow::Leave,
        _ => Follow::Nothing,
    }
}

/// Where a member goes (the follow rule's act, the prompt's Join, the
/// Leave dialog): `act` submits `connect` or `disconnect` unless
/// `PartyActs` is off (tests read the messages).
#[derive(Message, Clone, Debug, PartialEq, Eq)]
pub enum PartyGo {
    Join(Target),
    Leave,
}

/// Whether `PartyGo` is acted on (tests turn it off).
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct PartyActs(pub bool);

impl Default for PartyActs {
    fn default() -> Self {
        Self(true)
    }
}

fn act(mut go: MessageReader<PartyGo>, console: Option<ResMut<Console>>) {
    let Some(mut console) = console else {
        go.clear();
        return;
    };
    for g in go.read() {
        console.submit(match g {
            PartyGo::Join(t) => format!("connect {}", t.addr),
            PartyGo::Leave => "disconnect".to_string(),
        });
    }
}

/// Who we are to the party (the player's name and avatar: an app's
/// profile keeps it; the name from `NetSettings` otherwise).
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct PartyMe(pub Me);

/// `cl_party`: the party's public id (userinfo; empty alone).
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct PartyTag(pub String);

/// The userinfo key (`core::UserInfo::party` reads it).
pub const USERINFO_KEY: &str = crate::core::PARTY_KEY;

fn me(who: Option<&PartyMe>, net: Option<&super::NetSettings>) -> Me {
    let mut m = who.map(|w| w.0.clone()).unwrap_or(Me { name: String::new(), avatar: 255 });
    if m.name.trim().is_empty() {
        m.name = net.map(|n| n.name.clone()).filter(|n| !n.is_empty()).unwrap_or_else(|| "Player".into());
    }
    m
}

fn me_of(w: &World) -> Me {
    me(w.get_resource::<PartyMe>(), w.get_resource::<super::NetSettings>())
}

/// What the party's frame reads of this process.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Here<'w> {
    role: Option<Res<'w, NetRole>>,
    joined: Option<Res<'w, super::client::Joined>>,
    address: Option<Res<'w, super::client::ServerAddress>>,
    welcome: Option<Res<'w, super::client::ServerWelcome>>,
    udp: Option<Res<'w, super::udp::UdpServer>>,
    net: Option<Res<'w, super::NetSettings>>,
    hosting: Option<Res<'w, super::query::Hosting>>,
    who: Option<Res<'w, PartyMe>>,
    time: Option<Res<'w, Time<Real>>>,
}

impl Here<'_> {
    /// The server this process is on as a client with its map in, and its
    /// name.
    fn joined_at(&self) -> Option<(SocketAddr, String)> {
        if self.role.as_deref() != Some(&NetRole::Client) || self.joined.is_none() {
            return None;
        }
        let addr = self.address.as_ref()?.0;
        Some((addr, self.welcome.as_ref().map(|w| w.0.server_name.clone()).unwrap_or_default()))
    }

    /// A listen server's port and name, while this process hosts.
    fn hosting(&self) -> Option<(u16, String)> {
        if self.role.as_deref() != Some(&NetRole::Server) || self.udp.is_none() {
            return None;
        }
        Some((self.net.as_ref()?.hostport, self.hosting.as_ref().map(|h| h.hostname.clone()).unwrap_or_default()))
    }
}

/// The party's frame: answers taken, the follow rule applied, the
/// leader's place told, the heartbeat sent, `cl_party` kept.
fn party_tick(mut pc: ResMut<PartyClient>, here: Here, mut tag: ResMut<PartyTag>, mut go: MessageWriter<PartyGo>) {
    let pc = &mut *pc;
    let at = here.joined_at();
    let host = here.hosting();
    let at_text = at.as_ref().map(|(a, _)| a.to_string());
    let now = here.time.as_ref().map_or(0.0, |t| t.elapsed_secs_f64());
    let answers = pc.take_answers();
    let mut acts: Vec<Follow> = Vec::new();
    for a in answers {
        pc.busy = pc.busy.saturating_sub(1);
        pc.generation += 1;
        match a {
            Answer::Joined(Ok(j)) => {
                pc.token = Some(j.token);
                pc.handled = None;
                pc.told = None;
                pc.error = None;
                pc.keep();
                pc.view = Some(j.party);
                pc.poll_now();
            }
            Answer::View(token, r) if pc.token.as_deref() == Some(token.as_str()) => match r {
                Ok(v) => {
                    pc.error = None;
                    let act = follow(&v, pc.handled, at_text.as_deref(), host.is_some());
                    pc.handled = Some(v.moves);
                    if act != Follow::Nothing {
                        acts.push(act);
                    }
                    pc.view = Some(v);
                }
                Err(e) if party_doc::gone(&e) => {
                    pc.token = None;
                    pc.forget();
                    pc.note = Some("You're no longer in the party".into());
                }
                Err(e) => pc.error = Some(e.to_string()),
            },
            Answer::View(..) | Answer::Left(Ok(())) => {}
            Answer::Joined(Err(e)) | Answer::Left(Err(e)) => pc.error = Some(e.to_string()),
        }
    }
    for act in acts {
        match act {
            Follow::Join(t) => {
                pc.note = Some(format!("Joining your party at {}", if t.name.is_empty() { &t.addr } else { &t.name }));
                pc.prompt = None;
                go.write(PartyGo::Join(t));
            }
            Follow::Ask(t) => pc.prompt = Some(t),
            Follow::Leave => {
                pc.note = Some("Your party's leader left the match".into());
                pc.prompt = None;
                go.write(PartyGo::Leave);
            }
            Follow::Nothing => {}
        }
    }
    // The prompt goes once we are where it asks.
    if pc.prompt.as_ref().is_some_and(|t| at_text.as_deref() == Some(t.addr.as_str())) {
        pc.prompt = None;
    }
    // The leader's place.
    if pc.view.as_ref().is_some_and(PartyView::leading) {
        let place = match (&at, &host) {
            (Some((a, name)), _) => Some((Target { addr: a.to_string(), name: name.clone() }, false)),
            (None, Some((port, name))) => Some((Target { addr: format!(":{port}"), name: name.clone() }, true)),
            _ => None,
        };
        match place {
            Some((t, hosted)) if pc.told.as_deref() != Some(t.addr.as_str()) => pc.lead(Some(t), false, hosted),
            None => pc.told = None,
            _ => {}
        }
    }
    // The heartbeat.
    if pc.token.is_some() && pc.busy == 0 && now >= pc.next_poll {
        pc.next_poll = now + POLL_SECS as f64;
        let me = me(here.who.as_deref(), here.net.as_deref());
        pc.state(me, at_text.clone());
    }
    let want = pc.view.as_ref().map(|v| v.id.clone()).unwrap_or_default();
    if tag.0 != want {
        tag.0 = want;
    }
}

/// The user's hub's parties (an app's start, after `hub::use_user_hub`;
/// never in a tool run or a test).
pub fn use_user_party(mut commands: Commands, hub: Option<Res<super::hub::Hub>>, existing: Option<Res<PartyClient>>) {
    if existing.is_some_and(|p| p.available()) {
        return;
    }
    let Some(h) = hub.filter(|h| h.signed_in()) else { return };
    let Some(origin) = h.origin.clone() else { return };
    let http = HttpPartyHub { origin, fingerprint: h.fingerprint.clone(), account: h.account.clone() };
    let mut pc = PartyClient::new(Arc::new(http), PartyFile::path());
    pc.poll_now();
    commands.insert_resource(pc);
}

fn with_party<T>(w: &mut World, f: impl FnOnce(&mut World, &mut PartyClient) -> T) -> Result<T, String> {
    let Some(mut pc) = w.remove_resource::<PartyClient>() else { return Err("no parties here".into()) };
    if !pc.available() {
        w.insert_resource(pc);
        return Err("parties need a hub account (sign in to a hub first)".into());
    }
    let r = f(w, &mut pc);
    w.insert_resource(pc);
    Ok(r)
}

/// A member's id from its id or name (the party as last answered).
fn member_id(pc: &PartyClient, who: &str) -> Option<String> {
    let v = pc.view.as_ref()?;
    v.members.iter().find(|m| m.id == who || m.name.eq_ignore_ascii_case(who)).map(|m| m.id.clone())
}

/// `party`'s text.
pub fn describe(pc: &PartyClient) -> String {
    let Some(v) = pc.view.as_ref() else {
        return match &pc.error {
            Some(e) => format!("not in a party ({e})"),
            None if pc.token.is_some() => "in a party (asking the hub...)".into(),
            None => "not in a party".into(),
        };
    };
    let mut out = format!("party {} code {} ({} of {})", v.id, v.code, v.members.len(), party_doc::MAX_MEMBERS);
    for m in &v.members {
        out.push_str(&format!(
            "\n  {} {}{}{}{}",
            m.id,
            m.name,
            if m.leader { " [leader]" } else { "" },
            if m.id == v.you { " [you]" } else { "" },
            if m.online { "" } else { " (offline)" }
        ));
    }
    if let Some(t) = &v.target {
        out.push_str(&format!("\n  at {} {}", t.addr, t.name));
    }
    out
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<PartyTag>().init_resource::<PartyActs>().init_resource::<PartyMe>().add_message::<PartyGo>();
    resource_cvar::<PartyTag, String>(app, USERINFO_KEY, "Your party's id as servers see it (set by the party; empty alone).", |t| &mut t.0);
    app.world_mut().resource_mut::<Console>().userinfo(USERINFO_KEY);
    app.add_systems(Update, (party_tick.run_if(resource_exists::<PartyClient>), act.run_if(resource_equals(PartyActs(true)))).chain());
    let _ = app
        .console_command("party", "party: your party (its code, members, where it is).", |w, _| {
            Ok(Some(w.get_resource::<PartyClient>().map_or_else(|| "no parties here".into(), describe)))
        })
        .console_command("party_create", "party_create: make a party you lead (leaving yours).", |w, _| {
            { let m = me_of(w); with_party(w, |_, pc| pc.create(m)) }.map(|()| Some("making a party...".into()))
        })
        .console_command("party_join", "party_join <code>: join a party by its invite code.", |w, a| {
            let code = a.join("");
            if code.trim().is_empty() {
                return Err("party_join <code>".into());
            }
            { let m = me_of(w); with_party(w, |_, pc| pc.join(&code, m)) }.map(|()| Some(format!("joining party {code}...")))
        })
        .console_command("party_leave", "party_leave: leave your party.", |w, _| {
            with_party(w, |_, pc| pc.leave()).map(|()| Some("left the party".into()))
        })
        .console_command("party_kick", "party_kick <member id|name>: the leader puts a member out.", |w, a| {
            let who = a.join(" ");
            with_party(w, |_, pc| member_id(pc, &who).map(|m| pc.kick(&m)))?.ok_or_else(|| format!("no member {who:?}")).map(|()| None)
        })
        .console_command("party_promote", "party_promote <member id|name>: the leader hands the lead on.", |w, a| {
            let who = a.join(" ");
            with_party(w, |_, pc| member_id(pc, &who).map(|m| pc.promote(&m)))?.ok_or_else(|| format!("no member {who:?}")).map(|()| None)
        })
        .console_command("party_follow", "party_follow: join the server your party went to (the prompt's Join).", |w, _| {
            let t = with_party(w, |_, pc| pc.prompt.take().or_else(|| pc.view.as_ref().and_then(|v| v.target.clone())))?;
            let t = t.ok_or("the party isn't on a server")?;
            w.write_message(PartyGo::Join(t.clone()));
            Ok(Some(format!("joining your party at {}", t.addr)))
        })
        .console_command("party_stay", "party_stay: stay here (dismiss the party's prompt).", |w, _| {
            with_party(w, |_, pc| pc.prompt = None).map(|()| None)
        })
        .console_command(
            "party_leave_match",
            "party_leave_match [with|alone]: leave the match; the party's leader takes the party out too (with, the default) or goes alone.",
            |w, a| {
                let together = a.first().is_none_or(|x| x != "alone");
                if let Some(mut pc) = w.remove_resource::<PartyClient>() {
                    if pc.leads_others() {
                        pc.lead(None, together, false);
                    }
                    w.insert_resource(pc);
                }
                w.write_message(PartyGo::Leave);
                Ok(None)
            },
        );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view(leading: bool, moves: u32, kind: MoveKind, target: Option<&str>, left: Option<&str>) -> PartyView {
        PartyView {
            members: vec![
                MemberView { id: "l".into(), leader: true, ..default() },
                MemberView { id: "m".into(), ..default() },
            ],
            you: if leading { "l".into() } else { "m".into() },
            target: target.map(|a| Target { addr: a.into(), name: "S".into() }),
            moves,
            kind,
            left: left.map(str::to_string),
            ..default()
        }
    }

    #[test]
    fn members_follow_each_move_once() {
        let s = "192.0.2.1:27031";
        let t = Target { addr: s.into(), name: "S".into() };
        let join = view(false, 3, MoveKind::Join, Some(s), None);
        // On the menu: go; in another match or hosting: ask; there: nothing.
        assert_eq!(follow(&join, Some(2), None, false), Follow::Join(t.clone()));
        assert_eq!(follow(&join, Some(2), Some("192.0.2.9:1"), false), Follow::Ask(t.clone()));
        assert_eq!(follow(&join, Some(2), None, true), Follow::Ask(t.clone()));
        assert_eq!(follow(&join, Some(2), Some(s), false), Follow::Nothing);
        // Once only.
        assert_eq!(follow(&join, Some(3), None, false), Follow::Nothing);
        // The first answer acts on no old move: it asks.
        assert_eq!(follow(&join, None, None, false), Follow::Ask(t));
        // The leader never follows.
        assert_eq!(follow(&view(true, 3, MoveKind::Join, Some(s), None), Some(2), None, false), Follow::Nothing);
        // Leave with the party: only those on the server it left go.
        let out = view(false, 4, MoveKind::Leave, None, Some(s));
        assert_eq!(follow(&out, Some(3), Some(s), false), Follow::Leave);
        assert_eq!(follow(&out, Some(3), Some("192.0.2.9:1"), false), Follow::Nothing);
        assert_eq!(follow(&out, Some(3), None, false), Follow::Nothing);
        assert_eq!(follow(&out, None, Some(s), false), Follow::Nothing);
    }
}
