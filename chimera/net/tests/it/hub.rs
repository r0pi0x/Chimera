//! The hub from a game process (`chimera_net::net::hub`,
//! docs/plans/active/hub.md S2) against the real services on free
//! loopback ports over a scratch store (accounts, the hub, the front with
//! its certificate, as on a host): a headless dedicated server with
//! `chimera:hub 1` and a host account is listed at the address the hub
//! sees, reached by the hub's probe, updated when a bot joins and gone
//! when it quits; a server whose account may not list says why; a
//! client's `Hub` reads the signed document and the list, keeps them in
//! its cache and shows them, marked offline, when the hub is gone.

use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

use bevy::prelude::*;
use chimera_assets::{
    account::{self, HubLink},
    hub_doc::Reach,
    tls::{FrontLimits, Identity},
};
use chimera_core::core::{Bot, Intent};
use chimera_net::{
    dedicated,
    net::{
        NET_VERSION, NetSettings,
        hub::{Hub, HubGame, HubListing, ListState},
    },
};
use chimera_services::{
    accounts::{self, Accounts, Clock, InviteSpec, Invited},
    admin::Admin,
    front::{self, Routes},
    hub::{self as hub_service, Hub as HubService},
};
use chimera_world::harness::Sim;

use super::dedicated::{PLAIN_SERVER, server};

/// The app the test servers play on the hub.
const GAME: &str = "kit_party";

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("chimera_net_hub_{name}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// A host's services: accounts, the hub and the front before them.
struct Services {
    dir: PathBuf,
    store: PathBuf,
    clock: Clock,
    _accounts_run: accounts::http::Running,
    hub_run: Option<hub_service::http::Running>,
    front: front::Front,
    fingerprint: String,
    admin: Admin,
}

impl Services {
    fn new(name: &str) -> Self {
        let dir = scratch(name);
        let store = dir.join("server-store");
        let clock = Clock::default();
        let accounts = Arc::new(Accounts::open(accounts::Paths::new(store.clone()), accounts::Options::default(), clock.clone()).unwrap());
        let accounts_run = accounts::http::serve(accounts::http::Http::new(accounts.clone(), None), "127.0.0.1:0", 2, None).unwrap();
        let hub_run = Self::start_hub(&store, &clock, &accounts_run.url());
        let identity = Identity::from_seed(&[31; 32], [32; 16]);
        // (No asset service here: its paths go nowhere a test asks.)
        let routes = Routes { assets: hub_run.addr, accounts: accounts_run.addr, hub: hub_run.addr, feedback: hub_run.addr };
        let front = front::serve("127.0.0.1:0", &identity, routes, FrontLimits::default()).unwrap();
        let secret = fs::read_to_string(accounts.paths.secret()).unwrap();
        let admin = Admin::new(&accounts_run.url(), secret.trim());
        Self { dir, store, clock, _accounts_run: accounts_run, hub_run: Some(hub_run), front, fingerprint: identity.fingerprint(), admin }
    }

    fn start_hub(store: &Path, clock: &Clock, accounts_url: &str) -> hub_service::http::Running {
        let options = hub_service::Options { feed_every: Duration::from_millis(150), probe_wait: Duration::from_millis(800), ..Default::default() };
        let h = HubService::open(hub_service::Paths::new(store.to_path_buf()), options, clock.clone(), Some(accounts_url.to_string())).unwrap();
        hub_service::http::serve(hub_service::http::Http::new(h), "127.0.0.1:0", 2).unwrap()
    }

    fn hub(&self) -> &Arc<HubService> {
        &self.hub_run.as_ref().unwrap().http.hub
    }

    /// A new account signed in through the front (as a player's machine
    /// is): its account file's path.
    fn account(&self, name: &str, role: &str, list: Option<u32>) -> PathBuf {
        let invited: Invited =
            self.admin.post("/admin/invite", &InviteSpec { name: name.into(), role: Some(role.into()), list, ..Default::default() }).unwrap();
        let link = HubLink { host: self.front.addr.to_string(), fingerprint: Some(self.fingerprint.clone()), code: None };
        let (file, _) = account::redeem(&link, &invited.code, Some(name), "test").unwrap();
        let path = self.dir.join(format!("{name}-account.toml"));
        file.save_to(&path).unwrap();
        path
    }
}

/// A dedicated server on a free loopback UDP port, to be listed with
/// `account`.
fn listed_server(account: &Path) -> Sim {
    let mut sim = server(&PLAIN_SERVER, &[]);
    {
        let w = sim.app.world_mut();
        let mut s = w.resource_mut::<NetSettings>();
        s.ip = std::net::Ipv4Addr::LOCALHOST.into();
        s.hostport = 0;
        dedicated::listen(w).expect("a free UDP port");
        w.insert_resource(HubGame(GAME.into()));
        let mut l = w.resource_mut::<HubListing>();
        l.account = Some(account.to_path_buf());
        l.enabled = 1;
    }
    sim
}

fn port_of(sim: &Sim) -> u16 {
    sim.app.world().resource::<chimera_net::net::udp::UdpServer>().local_addr().unwrap().port()
}

/// Run `sims` (their heartbeats and query answers need frames) until `ok`.
fn pump(what: &str, sims: &mut [&mut Sim], mut ok: impl FnMut(&mut [&mut Sim]) -> bool) {
    let end = Instant::now() + Duration::from_secs(20);
    loop {
        for s in sims.iter_mut() {
            s.app.update();
        }
        if ok(sims) {
            return;
        }
        assert!(Instant::now() < end, "timed out: {what}");
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn state(sim: &Sim) -> ListState {
    sim.app.world().resource::<HubListing>().state.clone()
}

#[test]
fn a_dedicated_server_is_listed_updated_when_a_bot_joins_and_gone_on_quit() {
    let s = Services::new("listed");
    let host = s.account("hoster", "host", None);
    let mut sim = listed_server(&host);
    let port = port_of(&sim);
    // Listed at the address the hub saw, and reached by its probe (the
    // server answers the query on its game port).
    pump("listed and reached", &mut [&mut sim], |sims| {
        matches!(state(sims[0]), ListState::Listed(a) if a.reachable == Reach::Yes)
            || (matches!(state(sims[0]), ListState::Listed(_)) && s.hub().admin_list().iter().any(|l| l.listing.reachable == Reach::Yes))
    });
    let all = s.hub().admin_list();
    assert_eq!(all.len(), 1, "{all:?}");
    let l = &all[0].listing;
    assert_eq!(l.addr, format!("127.0.0.1:{port}"));
    assert_eq!((l.game.as_str(), l.net.as_str(), l.dedicated, l.players, l.bots), (GAME, NET_VERSION, true, 0, 0));
    assert_eq!(l.map, "greybox");
    assert_eq!(l.reachable, Reach::Yes);
    let status = chimera_net::net::status(sim.app.world_mut());
    assert!(status.contains(&format!("hub     : listed at 127.0.0.1:{port}")), "{status}");
    // A bot joins: the listing says so soon after (a change's heartbeat,
    // not the next half minute's).
    sim.app.world_mut().spawn((Intent::default(), Bot));
    let changed = Instant::now();
    pump("the bot listed", &mut [&mut sim], |_| s.hub().admin_list().first().is_some_and(|l| (l.listing.players, l.listing.bots) == (1, 1)));
    assert!(changed.elapsed() < Duration::from_secs(10), "a change is sent at once, not at the next beat");
    // `chimera:hub 0` delists at once; 1 lists again.
    sim.app.world_mut().resource_mut::<HubListing>().enabled = 0;
    pump("delisted", &mut [&mut sim], |_| s.hub().admin_list().is_empty());
    assert_eq!(state(&sim), ListState::Off);
    sim.app.world_mut().resource_mut::<HubListing>().enabled = 1;
    pump("listed again", &mut [&mut sim], |_| s.hub().admin_list().len() == 1);
    // Quitting delists (the quick exit's flush), not 90 s later.
    chimera_core::shutdown::run_steps(sim.app.world_mut());
    assert!(s.hub().admin_list().is_empty(), "gone on quit");
}

#[test]
fn a_server_that_may_not_list_says_why_and_one_without_an_account_too() {
    let s = Services::new("refused");
    let reader = s.account("reader", "tester", Some(0));
    let mut sim = listed_server(&reader);
    pump("refused", &mut [&mut sim], |sims| matches!(state(sims[0]), ListState::Failed(_)));
    let ListState::Failed(why) = state(&sim) else { unreachable!() };
    assert!(why.contains("may not list"), "{why}");
    assert!(s.hub().admin_list().is_empty());
    let status = chimera_net::net::status(sim.app.world_mut());
    assert!(status.contains("hub     : heartbeat failed"), "{status}");
    // No account file at all: said once, nothing sent.
    let mut none = listed_server(&s.dir.join("nobody.toml"));
    none.app.update();
    none.app.update();
    assert!(matches!(state(&none), ListState::NoAccount(_)), "{:?}", state(&none));
}

#[test]
fn a_client_reads_the_document_and_the_list_and_keeps_them_offline() {
    let mut s = Services::new("client");
    fs::create_dir_all(s.store.join("server/hub")).unwrap();
    fs::write(
        s.hub().paths.config(),
        "name = \"Test hub\"\nmotd = \"Playtest at 13:00\"\nmin_net = 9999\ndownload = \"https://example.invalid/r\"\n[[news]]\nid = \"a\"\ndate = \"2026-11-16\"\ntitle = \"New maps\"\ntext = \"Three of them.\"\n",
    )
    .unwrap();
    let host = s.account("hoster", "host", None);
    let reader = s.account("reader", "tester", Some(0));
    let mut sim = listed_server(&host);
    pump("listed", &mut [&mut sim], |_| s.hub().admin_list().iter().any(|l| l.listing.reachable == Reach::Yes));
    // The client's hub: its account's, nothing cached yet.
    let cache = s.dir.join("client").join("hub.toml");
    let mut hub = Hub::of_account(Some(reader.clone()), Some(cache.clone())).expect("signed in");
    assert!(hub.signed_in() && hub.doc.is_none() && hub.list.is_none());
    hub.refresh_list();
    pump("the document and the list", &mut [&mut sim], |_| {
        hub.poll();
        hub.doc.is_some() && hub.list.is_some() && !hub.busy
    });
    assert!(!hub.offline(), "{:?}", hub.error);
    let doc = hub.doc.clone().unwrap();
    assert_eq!((doc.name.as_str(), doc.motd.as_str()), ("Test hub", "Playtest at 13:00"));
    assert!(doc.too_old(NET_VERSION), "this build is under the hub's floor");
    assert_eq!(doc.newest_news(&hub.dismissed).map(|n| n.title.as_str()), Some("New maps"));
    let list = hub.list.clone().unwrap();
    assert_eq!(list.servers.len(), 1);
    assert_eq!((list.servers[0].game.as_str(), list.servers[0].mine), (GAME, false));
    assert_eq!(list.servers[0].addr, format!("127.0.0.1:{}", port_of(&sim)));
    // Dismissed news stays dismissed (kept in hub.toml).
    hub.dismiss("a");
    assert_eq!(doc.newest_news(&hub.dismissed), None);
    // The hub goes away: a new start shows the cached document at once
    // (it verifies with the pinned key), and a refresh says offline while
    // the document and the last list stay.
    s.hub_run = None;
    std::thread::sleep(Duration::from_millis(100));
    let mut again = Hub::of_account(Some(reader), Some(cache)).expect("signed in");
    assert_eq!(again.doc.as_ref().map(|d| d.name.as_str()), Some("Test hub"), "from the cache, before any fetch");
    assert_eq!(again.dismissed, vec!["a".to_string()]);
    hub.refresh_list();
    again.refresh_list();
    pump("the refusals", &mut [&mut sim], |_| {
        hub.poll();
        again.poll();
        !hub.busy && !again.busy && hub.offline() && again.offline()
    });
    assert!(hub.list.is_some() && hub.doc.is_some(), "the last list and the document stay");
    assert!(again.doc.is_some() && again.list.is_none());
    let _ = &s.clock;
}

// --- Parties (docs/plans/active/parties.md): two players' clients on a
// game server with the real hub behind the front.

mod parties {
    use std::net::SocketAddr;

    use bevy::ecs::message::MessageCursor;
    use chimera_assets::party_doc::MoveKind;
    use chimera_core::{console::Console, slots::Loadout};
    use chimera_net::{
        harness::NetSim,
        net::{
            client::ServerAddress,
            memory::LinkConditions,
            party::{HttpPartyHub, PartyActs, PartyClient, PartyGo},
        },
        prematch,
    };
    use chimera_sim::{SimBasePlugins, movement::placeholder};
    use chimera_world::greybox::GreyboxMapPlugin;

    use super::*;

    fn party_client(s: &Services, account: PathBuf) -> impl FnOnce(&mut World) {
        let hub = HttpPartyHub { origin: s.front.url(), fingerprint: Some(s.fingerprint.clone()), account: Some(account) };
        move |w| {
            w.insert_resource(PartyClient::new(Arc::new(hub), None));
            w.insert_resource(PartyActs(false));
        }
    }

    fn party(sim: &mut NetSim, i: usize) -> Option<chimera_assets::party_doc::PartyView> {
        sim.clients[i].app.world().resource::<PartyClient>().view.clone()
    }

    fn run(sim: &mut NetSim, what: &str, done: impl FnMut(&mut NetSim) -> bool) {
        let started = Instant::now();
        let mut done = done;
        while started.elapsed() < Duration::from_secs(20) {
            sim.step();
            if done(sim) {
                return;
            }
        }
        panic!("timed out: {what}");
    }

    fn submit(sim: &mut NetSim, i: usize, line: &str) {
        sim.clients[i].app.world_mut().resource_mut::<Console>().submit(line.to_string());
    }

    fn poll(sim: &mut NetSim, i: usize) {
        sim.clients[i].app.world_mut().resource_mut::<PartyClient>().poll_now();
    }

    /// Client `i`'s party asked again, and the answer in.
    fn refreshed(sim: &mut NetSim, i: usize) {
        let before = sim.clients[i].app.world().resource::<PartyClient>().generation;
        run(sim, "an answer", |sim| sim.clients[i].app.world().resource::<PartyClient>().busy == 0);
        poll(sim, i);
        run(sim, "the poll's answer", |sim| {
            let p = sim.clients[i].app.world().resource::<PartyClient>();
            p.generation > before && p.busy == 0
        });
    }

    /// The `PartyGo`s client `i` sent since `cursor`.
    fn gone(sim: &mut NetSim, i: usize, cursor: &mut MessageCursor<PartyGo>) -> Vec<PartyGo> {
        let m = sim.clients[i].app.world().resource::<Messages<PartyGo>>();
        cursor.read(m).cloned().collect()
    }

    #[test]
    fn a_party_made_by_code_is_grouped_on_the_server_and_follows_its_leader() {
        let s = Services::new("party_follow");
        let link = LinkConditions { latency: Duration::from_millis(20), jitter: Duration::ZERO, loss: 0.0 };
        let mut sim = NetSim::new(SimBasePlugins, link, 9, 0, |app| {
            app.add_plugins(GreyboxMapPlugin).insert_resource(Loadout { movement: placeholder::ID });
        });
        sim.add_client(party_client(&s, s.account("leader", "tester", Some(0))));
        sim.add_client(party_client(&s, s.account("member", "tester", Some(0))));
        sim.until_joined(600);
        // The leader makes a party; the member joins by its code.
        submit(&mut sim, 0, "party_create");
        run(&mut sim, "the party made", |sim| party(sim, 0).is_some());
        let code = party(&mut sim, 0).unwrap().code;
        submit(&mut sim, 1, &format!("party_join {}", code.to_lowercase()));
        run(&mut sim, "joined", |sim| party(sim, 1).is_some_and(|v| v.members.len() == 2));
        assert_eq!(party(&mut sim, 0).unwrap().id, party(&mut sim, 1).unwrap().id);
        // Both tell the server their party: one number in the lobby's rows.
        run(&mut sim, "grouped", |sim| {
            let rows = prematch::rows(sim.server.app.world_mut());
            rows.len() == 2 && rows[0].party != 0 && rows[0].party == rows[1].party
        });
        // The leader is on a server (its address as a real client's): the
        // hub hears it, and the member on the menu goes there.
        let server: SocketAddr = "192.0.2.10:27031".parse().unwrap();
        let mut cursor = MessageCursor::<PartyGo>::default();
        let _ = gone(&mut sim, 1, &mut cursor);
        sim.clients[0].app.world_mut().insert_resource(ServerAddress(server));
        run(&mut sim, "the leader's move told", |sim| party(sim, 0).is_some_and(|v| v.kind == MoveKind::Join && v.moves == 1));
        refreshed(&mut sim, 1);
        sim.step();
        let went = gone(&mut sim, 1, &mut cursor);
        assert!(matches!(&went[..], [PartyGo::Join(t)] if t.addr == server.to_string()), "{went:?}");
        // There now; the leader leaves with the party: the member goes back.
        sim.clients[1].app.world_mut().insert_resource(ServerAddress(server));
        refreshed(&mut sim, 1);
        refreshed(&mut sim, 0);
        let there = party(&mut sim, 0).unwrap();
        assert!(there.members.iter().any(|m| !m.leader && m.at.as_deref() == Some("192.0.2.10:27031")), "{there:?}");
        submit(&mut sim, 0, "party_leave_match");
        sim.step();
        // (Its disconnect, which the test's `PartyActs(false)` leaves out.)
        sim.clients[0].app.world_mut().remove_resource::<ServerAddress>();
        run(&mut sim, "the leave told", |sim| party(sim, 0).is_some_and(|v| v.kind == MoveKind::Leave));
        refreshed(&mut sim, 1);
        sim.step();
        let went = gone(&mut sim, 1, &mut cursor);
        assert_eq!(went, [PartyGo::Leave]);
        // The member leaves the party: the leader is alone again.
        submit(&mut sim, 1, "party_leave");
        run(&mut sim, "left", |sim| sim.clients[1].app.world().resource::<PartyClient>().busy == 0);
        refreshed(&mut sim, 0);
        assert_eq!(party(&mut sim, 0).unwrap().members.len(), 1);
        assert!(party(&mut sim, 1).is_none());
        assert_eq!(s.hub().party_count(), 1);
    }
}
