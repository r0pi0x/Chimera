//! The accounts service with the asset service beside it, both on free
//! loopback ports over one scratch server store (as on the host): the
//! owner's invites through the admin API, a player's redeem, link codes,
//! recovery and tickets through the client side (`chimera_assets::account`),
//! downloads with tickets, revocation through the feed; tickets as the
//! hub's only way in.

use std::{
    fs,
    sync::Arc,
    time::{Duration, Instant},
};

use chimera_assets::{
    FileRecord, Manifest, Remote, RemoteConfig, Store,
    account::{self, AccountFile, HubLink, RedeemRequest, Session, TicketRequest},
    server,
    store::hex,
    ticket,
};
use chimera_services::{
    accounts::{
        AccountView, Accounts, Clock, InviteSpec, Invited, Options, Paths,
        http::{Http, Running, serve},
    },
    admin::Admin,
};
use ed25519_dalek::{Signer, SigningKey};

use super::scratch;

/// How often the asset service polls the revocation feed here.
const FEED: Duration = Duration::from_millis(200);

struct Hub {
    dir: std::path::PathBuf,
    paths: Paths,
    clock: Clock,
    running: Running,
    assets: server::Running,
    admin: Admin,
    /// A record of game `fixture` and one of game `other`.
    records: (String, String),
}

fn publish(store: &Store, game: &str, bytes: &[u8]) -> String {
    let put = store.put(bytes).unwrap();
    let mut m = Manifest::new(game, "1", None);
    m.files.insert("data/file.txt".into(), FileRecord { id: put.id.clone(), size: bytes.len() as u64, stored: put.stored });
    m.save(&store.manifest_path(game, "1")).unwrap();
    put.id
}

impl Hub {
    fn new(name: &str) -> Self {
        Self::with(name, Options::default())
    }

    fn with(name: &str, options: Options) -> Self {
        let dir = scratch(name);
        let store = Store::new(dir.join("server-store"));
        let records = (publish(&store, "fixture", b"fixture's bytes"), publish(&store, "other", b"other game's bytes"));
        let paths = Paths::new(store.root.clone());
        let clock = Clock::default();
        let accounts = Arc::new(Accounts::open(paths.clone(), options, clock.clone()).unwrap());
        let running = serve(Http::new(accounts, None), "127.0.0.1:0", 2, None).unwrap();
        let service = server::Service::with_tickets(store, &running.url(), FEED).unwrap();
        let assets = server::serve(service, "127.0.0.1:0", 2).unwrap();
        let secret = fs::read_to_string(paths.secret()).unwrap();
        let admin = Admin::new(&running.url(), secret.trim());
        Self { dir, paths, clock, running, assets, admin, records }
    }

    fn invite(&self, spec: InviteSpec) -> Invited {
        self.admin.post("/admin/invite", &spec).unwrap()
    }

    fn invite_named(&self, name: &str) -> String {
        self.invite(InviteSpec { name: name.into(), content: Some("fixture".into()), ..Default::default() }).code
    }

    fn link(&self) -> HubLink {
        HubLink { host: self.running.addr.to_string(), fingerprint: None, code: None }
    }

    fn redeem(&self, code: &str, username: Option<&str>) -> Result<(AccountFile, Option<String>), String> {
        account::redeem(&self.link(), code, username, "test pc")
    }

    fn remote(&self, token: &str) -> Remote {
        Remote::new(RemoteConfig { url: self.assets.url(), token: token.into(), key: None, fingerprint: None }).unwrap()
    }

    fn accounts(&self) -> Vec<AccountView> {
        self.admin.get("/admin/accounts").unwrap()
    }

    /// Wait (at most `limit`) until `f` holds.
    fn until(&self, limit: Duration, mut f: impl FnMut() -> bool) -> bool {
        let end = Instant::now() + limit;
        while Instant::now() < end {
            if f() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        f()
    }
}

impl Drop for Hub {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

fn status(e: &str) -> u16 {
    // "the hub refused: <words> (<status>)"
    e.rsplit('(').next().and_then(|s| s.trim_end_matches(')').parse().ok()).unwrap_or(0)
}

/// The whole path: the owner invites, the player redeems with a fresh
/// device key, the ticket downloads the invite's content only; a second
/// redeem of the code is refused; a refreshed ticket (the device's signed
/// request) works too.
#[test]
fn invite_redeem_ticket_and_download() {
    let hub = Hub::new("flow");
    let code = hub.invite_named("Alice");
    let (file, recovery) = hub.redeem(&code, None).unwrap();
    assert_eq!((file.username.as_str(), file.display_name.as_str()), ("alice", "alice"), "the invite's name, folded");
    let recovery = recovery.expect("a recovery key at sign-up");
    assert_eq!(recovery.len(), 19);
    let ticket = file.ticket.clone().unwrap();
    let claims = hub.running.http.accounts.ticket_keys().check(&ticket, ticket::SERVICES, hub.clock.now()).unwrap();
    assert_eq!((claims.username.as_str(), claims.content.clone()), ("alice", vec!["fixture".to_string()]));

    // The asset service: this content only.
    let remote = hub.remote(&ticket);
    assert!(remote.whoami().unwrap().starts_with("account alice 0\ngames fixture"));
    let games: Vec<String> = remote.games().unwrap().into_iter().map(|g| g.0).collect();
    assert_eq!(games, ["fixture"]);
    assert!(remote.record(&hub.records.0).is_ok());
    let e = remote.record(&hub.records.1).unwrap_err();
    assert_eq!(e.kind(), std::io::ErrorKind::PermissionDenied, "{e}");

    // A code is single use.
    let e = hub.redeem(&code, Some("alice2")).unwrap_err();
    assert_eq!(status(&e), 410, "{e}");

    // A ticket asked with the device key (a second later: another ticket).
    hub.clock.advance(1);
    let fresh = account::request_ticket(&file).unwrap();
    assert_ne!(fresh, ticket);
    assert!(hub.remote(&fresh).whoami().is_ok());
    // A changed byte: refused at the asset service.
    let mut forged = fresh.clone().into_bytes();
    let i = forged.len() - 3;
    forged[i] = if forged[i] == b'A' { b'B' } else { b'A' };
    let e = hub.remote(&String::from_utf8(forged).unwrap()).whoami().unwrap_err();
    assert_eq!(e.kind(), std::io::ErrorKind::PermissionDenied);
}

/// `+chimera:hub <link>`: the file and the recovery key beside it, the
/// session live, `Remote` sending its ticket; through the TLS front with
/// the link's fingerprint pinned.
#[test]
fn a_hub_link_signs_in_through_pinned_tls() {
    let hub = Hub::new("join");
    let identity = chimera_assets::tls::Identity::load_or_create(&hub.paths.tls()).unwrap();
    let front =
        chimera_assets::tls::front("127.0.0.1:0", hub.running.addr, &identity, Default::default(), Default::default()).unwrap();
    let code = hub.invite_named("bob");
    let link = HubLink { host: front.url().trim_start_matches("https://").into(), fingerprint: Some(identity.fingerprint()), code: Some(code) };
    let file_path = hub.dir.join("client").join("account.toml");
    let said = account::join_hub(&link.to_text(true), Some("Bobby"), &file_path).unwrap();
    assert!(said.contains("as bobby") && said.contains("recovery-key.txt"), "{said}");
    assert!(!said.contains(&link.code.clone().unwrap()), "the code isn't echoed");
    let file = AccountFile::load_from(&file_path).unwrap().unwrap();
    assert_eq!((file.username.as_str(), file.fingerprint.as_deref()), ("bobby", Some(identity.fingerprint().as_str())));
    let recovery = fs::read_to_string(file_path.with_file_name("recovery-key.txt")).unwrap();
    assert_eq!(recovery.lines().nth(1).unwrap().len(), 19);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(fs::metadata(&file_path).unwrap().permissions().mode() & 0o777, 0o600);
    }
    // The session refreshes over the pinned link; another fingerprint is refused.
    let session = Session::new(file.clone(), Some(file_path.clone()));
    assert!(session.refresh().is_ok());
    let wrong = AccountFile { fingerprint: Some("00".repeat(32)), ..file };
    let e = account::request_ticket(&wrong).unwrap_err();
    assert!(matches!(account::Failure::of(&e), account::Failure::Certificate { offered: Some(_) }), "the onboarding's mismatch: {e}");
    // A link to a closed port: unreachable.
    let closed = HubLink { host: "127.0.0.1:9".into(), fingerprint: Some(identity.fingerprint()), code: None };
    let e = account::check(&closed, "AAA-AAA-AAA", None).unwrap_err();
    assert!(matches!(account::Failure::of(&e), account::Failure::Unreachable(_)), "{e}");
    // A plain config with the live session's ticket: `Remote` sends a current one.
    let live = account::live().expect("join_hub made the session live");
    let remote = hub.remote(&live.ticket().unwrap());
    assert!(remote.whoami().unwrap().starts_with("account bobby"));
    account::set_live(None);
}

/// Revoking a device stops its tickets at the asset service within one
/// poll of the feed, and it gets no new ones.
#[test]
fn a_revoked_device_is_refused_within_the_feed_interval() {
    let hub = Hub::new("revoke");
    let (file, _) = hub.redeem(&hub.invite_named("carol"), None).unwrap();
    let ticket = file.ticket.clone().unwrap();
    assert!(hub.remote(&ticket).whoami().is_ok());
    hub.admin.post::<()>("/admin/device/revoke", &serde_json::json!({ "device": file.device })).unwrap();
    let started = Instant::now();
    assert!(
        hub.until(FEED * 5, || hub.remote(&ticket).whoami().is_err()),
        "still served after {:?}",
        started.elapsed()
    );
    let e = hub.remote(&ticket).whoami().unwrap_err();
    assert!(e.to_string().contains("revoked"), "{e}");
    assert_eq!(status(&account::request_ticket(&file).unwrap_err()), 401);
    // Revoking the whole account: the same for its other devices.
    let link: account::LinkAnswer = {
        let (second, _) = hub.redeem(&hub.invite_named("dave"), None).unwrap();
        let t = second.ticket.clone().unwrap();
        account::link_code(&second, &t).unwrap()
    };
    let (third, _) = hub.redeem(&link.code, None).unwrap();
    assert_eq!(third.username, "dave");
    let t = third.ticket.clone().unwrap();
    assert!(hub.remote(&t).whoami().is_ok());
    hub.admin.post::<()>("/admin/account/revoke", &serde_json::json!({ "username": "dave" })).unwrap();
    assert!(hub.until(FEED * 5, || hub.remote(&t).whoami().is_err()));
}

/// An expired, revoked or used invite and a used or expired link code are
/// refused; a wrong code counts toward the address's limit.
#[test]
fn expired_invites_and_reused_codes_are_refused() {
    let hub = Hub::with("expiry", Options { failures_per_hour: 4, ..Options::default() });
    let short = hub.invite(InviteSpec { name: "erin".into(), expires_days: Some(1), ..Default::default() }).code;
    let revoked = hub.invite_named("frank");
    let n: usize = hub.admin.post("/admin/invite/revoke", &serde_json::json!({ "name": "frank" })).unwrap();
    assert_eq!(n, 1);
    assert_eq!(status(&hub.redeem(&revoked, None).unwrap_err()), 410);
    let (file, _) = hub.redeem(&hub.invite_named("gina"), None).unwrap();
    let link = account::link_code(&file, file.ticket.as_deref().unwrap()).unwrap();
    assert_eq!(link.code.len(), 7);
    let (second, recovery) = hub.redeem(&link.code.to_lowercase(), None).unwrap();
    assert!(recovery.is_none(), "a link code adds a device, no new recovery key");
    assert_eq!((second.account.as_str(), second.username.as_str()), (file.account.as_str(), "gina"));
    assert_ne!(second.device, file.device);
    assert_eq!(status(&hub.redeem(&link.code, None).unwrap_err()), 410, "a link code is single use");
    let late = account::link_code(&file, file.ticket.as_deref().unwrap()).unwrap();
    hub.clock.advance(601);
    assert_eq!(status(&hub.redeem(&late.code, None).unwrap_err()), 410, "10 minutes");
    hub.clock.advance(86_400);
    assert_eq!(status(&hub.redeem(&short, None).unwrap_err()), 410, "the invite expired");
    let views: Vec<chimera_services::accounts::InviteView> = hub.admin.get("/admin/invites").unwrap();
    let state = |n: &str| views.iter().find(|v| v.name == n).unwrap().state.clone();
    assert_eq!((state("erin"), state("frank"), state("gina")), ("expired".into(), "revoked".into(), "redeemed".into()));
    // Wrong codes in the last hour (the expired invite and three unknown
    // codes) use up this address's tries: even a good one waits.
    for _ in 0..3 {
        assert_eq!(status(&hub.redeem("AAA-AAA-AAA", None).unwrap_err()), 404);
    }
    let good = hub.invite_named("hank");
    assert_eq!(status(&hub.redeem(&good, None).unwrap_err()), 429);
    hub.clock.advance(3601);
    assert!(hub.redeem(&good, None).is_ok());
}

/// Usernames: 3-20 of letters, digits and `_`, case-insensitive, unique,
/// not reserved; a refused one leaves the code usable. Display names: the
/// username by default, at most 20 characters.
#[test]
fn username_rules() {
    let hub = Hub::new("usernames");
    let code = hub.invite_named("someone");
    for (bad, want) in [("ab", 400), (&"x".repeat(21) as &str, 400), ("has space", 400), ("admin", 400)] {
        assert_eq!(status(&hub.redeem(&code, Some(bad)).unwrap_err()), want, "{bad:?}");
    }
    let (first, _) = hub.redeem(&code, Some("Mixed_Case")).unwrap();
    assert_eq!(first.username, "mixed_case");
    let code = hub.invite_named("other");
    let e = hub.redeem(&code, Some("MIXED_case")).unwrap_err();
    assert_eq!(status(&e), 409, "case-insensitive: {e}");
    let twenty = "y".repeat(20);
    let (second, _) = hub.redeem(&code, Some(&twenty)).unwrap();
    assert_eq!((second.username.as_str(), second.display_name.as_str()), (twenty.as_str(), twenty.as_str()));
    // An invite whose name isn't a username needs one given.
    let code = hub.invite_named("Ivy Smith");
    assert_eq!(status(&hub.redeem(&code, None).unwrap_err()), 400);
    assert!(hub.redeem(&code, Some("ivy")).is_ok());
    // Display names: at most 20 characters, set with a ticket; no hidden
    // characters, look-alikes or another's username; the next ticket says it.
    let t = first.ticket.clone().unwrap();
    let set = |name: &str| account::set_display_name(&first, &t, name).and_then(account::view);
    assert_eq!(status(&set(&"z".repeat(21)).unwrap_err()), 400);
    for trick in ["Ivy\u{200b}", "\u{202e}ESAC", "@ivy", "Мixed", "IVY", " i v y"] {
        let e = set(trick).unwrap_err();
        assert_eq!(status(&e), 400, "{trick:?}: {e}");
    }
    let me = set("  The   Mixed One  ").unwrap();
    assert_eq!((me.username.as_str(), me.display_name.as_str()), ("mixed_case", "The Mixed One"));
    assert_eq!(set("Ivy and me").unwrap().display_name, "Ivy and me", "not unique, only not a username");
    assert_eq!(set("Mixed Case").unwrap().display_name, "Mixed Case", "its own username reads fine");
    set("The Mixed One").unwrap();
    let mine = hub.accounts().into_iter().find(|a| a.username == "mixed_case").unwrap();
    assert_eq!(mine.display_name, "The Mixed One");
    assert_eq!(chimera_assets::ticket::peek(&t).unwrap().display_name, "mixed_case", "the old ticket: as it was issued");
    let fresh = account::request_ticket(&first).unwrap();
    assert_eq!(chimera_assets::ticket::peek(&fresh).unwrap().display_name, "The Mixed One");
}

/// Recovery: a wrong key counts toward the limit; the right key adds a
/// device, revokes the others and replaces the key. A re-invite into the
/// account keeps its username.
#[test]
fn recovery_and_reinvites() {
    let hub = Hub::with("recovery", Options { failures_per_hour: 3, ..Options::default() });
    let (old, recovery) = hub.redeem(&hub.invite_named("jack"), None).unwrap();
    let recovery = recovery.unwrap();
    let e = account::recover(&hub.link(), "jack", "0000-0000-0000-0000", "new pc").unwrap_err();
    assert_eq!(status(&e), 403, "{e}");
    let (new, again) = account::recover(&hub.link(), "JACK", &recovery.to_lowercase(), "new pc").unwrap();
    assert_eq!((new.account.as_str(), new.username.as_str()), (old.account.as_str(), "jack"));
    let again = again.unwrap();
    assert_ne!(again, recovery, "a new recovery key");
    assert_eq!(status(&account::request_ticket(&old).unwrap_err()), 401, "the old PC's device is revoked");
    assert!(account::request_ticket(&new).is_ok());
    assert_eq!(status(&account::recover(&hub.link(), "jack", &recovery, "x").unwrap_err()), 403, "the old key is gone");

    // A re-invite: the same account and username, other devices revoked.
    let reinvite = hub.invite(InviteSpec { name: "jack again".into(), for_username: Some("jack".into()), ..Default::default() });
    let (back, key) = hub.redeem(&reinvite.code, Some("ignored")).unwrap();
    assert_eq!((back.account.as_str(), back.username.as_str()), (old.account.as_str(), "jack"));
    assert!(key.as_ref().is_some_and(|k| *k != again));
    assert_eq!(status(&account::request_ticket(&new).unwrap_err()), 401);
    // With --keep, the others stay.
    let keep = hub.invite(InviteSpec { name: "jack 3".into(), for_username: Some("jack".into()), keep: true, ..Default::default() });
    let (third, latest) = hub.redeem(&keep.code, None).unwrap();
    account::request_ticket(&back).unwrap();
    account::request_ticket(&third).unwrap();
    let jack = hub.accounts().into_iter().find(|a| a.username == "jack").unwrap();
    assert_eq!(jack.devices.iter().filter(|d| !d.revoked).count(), 2);
    // Wrong keys up to the limit (3 with the old key's try above): the
    // right one waits an hour.
    let again = latest.unwrap();
    let _ = account::recover(&hub.link(), "jack", "1111-1111-1111-1111", "x");
    assert_eq!(status(&account::recover(&hub.link(), "jack", &again, "x").unwrap_err()), 429);
    hub.clock.advance(3601);
    assert!(account::recover(&hub.link(), "jack", &again, "x").is_ok());
}

/// A ticket request with an old time or a nonce seen before is refused.
#[test]
fn ticket_requests_are_fresh_and_once() {
    let hub = Hub::new("nonce");
    let key = SigningKey::from_bytes(&[5; 32]);
    let request = RedeemRequest {
        code: hub.invite_named("kim"),
        username: None,
        device_public_key: hex(key.verifying_key().as_bytes()),
        device_label: "t".into(),
        avatar: None,
        display_name: None,
    };
    let signed = hub.running.http.accounts.redeem(&request, None).unwrap();
    let device = ticket::parse_id(&signed.device).unwrap();
    let ask = |time: u64, nonce: [u8; 16]| {
        let signature = key.sign(&ticket::request_message(&device, time, &nonce));
        let r = TicketRequest { device: signed.device.clone(), time, nonce: hex(&nonce), signature: hex(&signature.to_bytes()) };
        hub.running.http.accounts.ticket(&r, None)
    };
    let now = hub.clock.now();
    assert!(ask(now, [1; 16]).is_ok());
    assert_eq!(ask(now, [1; 16]).unwrap_err().status, 401, "a seen nonce");
    assert_eq!(ask(now - 301, [2; 16]).unwrap_err().status, 401, "too old");
    assert_eq!(ask(now + 301, [3; 16]).unwrap_err().status, 401, "too new");
    assert!(ask(now - 200, [4; 16]).is_ok());
    // Another key's signature.
    let other = SigningKey::from_bytes(&[6; 32]);
    let nonce = [7; 16];
    let bad = other.sign(&ticket::request_message(&device, now, &nonce));
    let r = TicketRequest { device: signed.device.clone(), time: now, nonce: hex(&nonce), signature: hex(&bad.to_bytes()) };
    assert_eq!(hub.running.http.accounts.ticket(&r, None).unwrap_err().status, 401);
}

/// A hub's asset service takes account tickets only: a token in the
/// store's `accounts.toml` (what an asset service with no hub reads,
/// `Service::new`) gets nothing there, and the accounts service gives
/// tickets for a device's signed request alone, never for a bearer token.
#[test]
fn tickets_are_the_hubs_only_way_in() {
    let hub = Hub::new("tickets-only");
    let path = chimera_assets::accounts::Accounts::path(&hub.paths.store);
    let mut tokens = chimera_assets::accounts::Accounts::default();
    let (_, token) = tokens.add("someone", Some(vec!["fixture".into()]), "pc").unwrap();
    tokens.save(&path).unwrap();
    assert!(hub.remote(&token).whoami().is_err(), "the hub's asset service has no tokens");
    assert!(hub.remote(&token).record(&hub.records.0).is_err());
    // The same store served with no hub takes the token.
    let plain = server::serve(server::Service::new(Store::new(hub.paths.store.clone())).unwrap(), "127.0.0.1:0", 1).unwrap();
    let remote = Remote::new(RemoteConfig { url: plain.url(), token: token.clone(), key: None, fingerprint: None }).unwrap();
    assert!(remote.whoami().unwrap().starts_with("internal someone 1"));
    // A bearer token at `/accounts/ticket` is no request.
    let answer = ureq::post(&format!("{}/accounts/ticket", hub.running.url()))
        .header("Authorization", &format!("Bearer {token}"))
        .config()
        .http_status_as_error(false)
        .build()
        .send_empty()
        .unwrap();
    assert_eq!(answer.status().as_u16(), 400);
}

/// The admin API answers only this machine with the secret; other paths
/// are 421; nothing is anonymous.
#[test]
fn the_admin_api_wants_the_secret() {
    let hub = Hub::new("admin");
    let wrong = Admin::new(&hub.running.url(), "not the secret");
    assert!(wrong.get::<Vec<AccountView>>("/admin/accounts").unwrap_err().contains("404"));
    assert!(Admin::new(&hub.running.url(), "").get::<String>("/internal/revoked").unwrap_err().contains("404"));
    let agent = chimera_assets::remote::agent(None).unwrap();
    let get = |path: &str| agent.get(&format!("{}{path}", hub.running.url())).call().unwrap().status().as_u16();
    assert_eq!(get("/accounts/me"), 401);
    assert_eq!(get("/m/fixture/1"), 421, "not this service's path");
    assert_eq!(get("/health"), 200, "loopback");
    let backup: serde_json::Value = hub.admin.post("/admin/backup", &serde_json::json!({})).unwrap();
    let path = std::path::PathBuf::from(backup["path"].as_str().unwrap());
    assert!(path.exists() && path.starts_with(hub.paths.backups()));
    chimera_services::db::integrity(&path).unwrap();
}

/// Codes, recovery keys, tickets and device keys never reach the logs,
/// the audit rows or the database in the clear.
#[test]
fn no_secrets_are_logged_or_stored() {
    let hub = Hub::new("secrets");
    let code = hub.invite_named("lena");
    let (file, recovery) = hub.redeem(&code, None).unwrap();
    let recovery = recovery.unwrap();
    let ticket = file.ticket.clone().unwrap();
    let link = account::link_code(&file, &ticket).unwrap();
    let (second, _) = hub.redeem(&link.code, None).unwrap();
    let _ = account::recover(&hub.link(), "lena", "2222-2222-2222-2222", "x");
    hub.remote(&ticket).whoami().unwrap();
    let fresh = account::request_ticket(&second).unwrap();
    hub.admin.post::<serde_json::Value>("/admin/backup", &serde_json::json!({})).unwrap();
    drop(hub.running.http.accounts.list_accounts());
    let secrets = [
        code.clone(),
        code.replace('-', ""),
        recovery.clone(),
        recovery.replace('-', ""),
        link.code.clone(),
        link.code.replace('-', ""),
        ticket.clone(),
        ticket[ticket::PREFIX.len()..ticket::PREFIX.len() + 40].to_string(),
        fresh[ticket::PREFIX.len()..ticket::PREFIX.len() + 40].to_string(),
        file.device_key.clone(),
        second.device_key.clone(),
    ];
    let mut places: Vec<(String, Vec<u8>)> = Vec::new();
    for dir in [hub.paths.server()] {
        let mut stack = vec![dir];
        while let Some(d) = stack.pop() {
            for e in fs::read_dir(&d).unwrap().flatten() {
                if e.path().is_dir() {
                    stack.push(e.path());
                } else {
                    places.push((e.path().display().to_string(), fs::read(e.path()).unwrap()));
                }
            }
        }
    }
    let accounts_json = serde_json::to_vec(&hub.accounts()).unwrap();
    places.push(("admin account list".into(), accounts_json));
    assert!(places.iter().any(|(p, _)| p.ends_with("access.log")), "logs checked");
    assert!(places.iter().any(|(p, _)| p.ends_with("accounts.db")), "the database checked");
    for (place, bytes) in &places {
        let text = String::from_utf8_lossy(bytes);
        for s in &secrets {
            assert!(!text.contains(s.as_str()), "a secret ({}...) in {place}", &s[..4]);
        }
    }
}

/// The owner's account commands: `set` changes rights (the old ticket
/// keeps the old ones), `refresh` makes the devices ask again at once (the
/// old ticket refused within a feed poll, a new one asked in the same
/// second carries the new rights), `show` lists the changes, a username
/// change is refused when taken, `delete` stops its tickets and frees the
/// name.
#[test]
fn the_owner_sets_shows_refreshes_and_deletes_accounts() {
    use chimera_services::accounts::{AccountChange, AccountDetail};
    let hub = Hub::new("owner_cmds");
    let (file, _) = hub.redeem(&hub.invite_named("Frank"), None).unwrap();
    let ticket = file.ticket.clone().unwrap();
    let change = AccountChange {
        username: "frank".into(),
        role: Some("host".into()),
        content: Some("fixture,other".into()),
        note: Some("runs the Tuesday server".into()),
        muted: Some(true),
        ..Default::default()
    };
    let a: AccountView = hub.admin.post("/admin/account/set", &change).unwrap();
    assert_eq!((a.role.as_str(), a.content.as_str(), a.list_slots, a.report, a.muted), ("host", "fixture,other", 4, true, true));
    let e = hub.remote(&ticket).record(&hub.records.1).unwrap_err();
    assert_eq!(e.kind(), std::io::ErrorKind::PermissionDenied, "the old ticket has the old rights: {e}");

    hub.admin.post::<()>("/admin/account/refresh", &serde_json::json!({ "username": "frank" })).unwrap();
    assert!(hub.until(Duration::from_secs(5), || hub.remote(&ticket).whoami().is_err()), "the old ticket stops");
    let fresh = account::request_ticket(&file).unwrap();
    assert!(hub.remote(&fresh).record(&hub.records.1).is_ok(), "the new ticket has the new rights");

    let d: AccountDetail = hub.admin.get("/admin/account?username=Frank").unwrap();
    let actions: Vec<&str> = d.audit.iter().map(|e| e.action.as_str()).collect();
    for want in ["invite.redeem", "account.set", "account.refresh"] {
        assert!(actions.contains(&want), "{want} in {actions:?}");
    }
    assert_eq!(d.account.note, "runs the Tuesday server");

    let _ = hub.redeem(&hub.invite_named("Gina"), None).unwrap();
    let taken = AccountChange { username: "frank".into(), new_username: Some("gina".into()), ..Default::default() };
    assert!(hub.admin.post::<AccountView>("/admin/account/set", &taken).unwrap_err().contains("409"));
    let long = AccountChange { username: "frank".into(), display_name: Some("x".repeat(21)), ..Default::default() };
    assert!(hub.admin.post::<AccountView>("/admin/account/set", &long).unwrap_err().contains("400"));
    let renamed = AccountChange { username: "frank".into(), new_username: Some("Franky".into()), ..Default::default() };
    assert_eq!(hub.admin.post::<AccountView>("/admin/account/set", &renamed).unwrap().username, "franky");

    hub.admin.post::<()>("/admin/account/delete", &serde_json::json!({ "username": "franky" })).unwrap();
    assert!(!hub.accounts().iter().any(|a| a.username == "franky"));
    assert!(hub.until(Duration::from_secs(5), || hub.remote(&fresh).whoami().is_err()), "a deleted account's tickets stop");
    assert!(account::request_ticket(&file).is_err(), "and it gets no new one");
    let (again, _) = hub.redeem(&hub.invite_named("someone"), Some("franky")).unwrap();
    assert_eq!(again.username, "franky", "the username is free again");
}

/// The admin page: served to this machine with a local `Host` only; it
/// holds no secret (its calls need one).
#[test]
fn the_admin_page_is_for_this_machine() {
    let hub = Hub::new("page");
    let agent = chimera_assets::remote::agent(None).unwrap();
    let mut r = agent.get(&format!("{}/admin/", hub.running.url())).call().unwrap();
    assert_eq!(r.status().as_u16(), 200);
    let page = r.body_mut().read_to_string().unwrap();
    assert!(page.contains("Hub accounts") && page.contains("X-Chimera-Service"));
    let secret = fs::read_to_string(hub.paths.secret()).unwrap();
    assert!(!page.contains(secret.trim()), "the page holds no secret");
    let rebound = agent.get(&format!("{}/admin/", hub.running.url())).header("Host", "evil.example").call().unwrap();
    assert_eq!(rebound.status().as_u16(), 404, "another Host (DNS rebinding)");
}

/// Onboarding (docs/plans/active/onboarding.md): an invite with an intent
/// carries no username; `check` shows the code's kind, the hub's name and
/// MOTD and a name's availability with suggestions, using nothing up; the
/// player's chosen name and avatar make the account, which keeps the
/// invite's intent.
#[test]
fn an_invite_with_an_intent_wants_a_chosen_name() {
    let hub = Hub::with("intent", Options { failures_per_hour: 5, ..Options::default() });
    let hub_dir = hub.paths.store.join("server").join("hub");
    fs::create_dir_all(&hub_dir).unwrap();
    fs::write(hub_dir.join("hub.toml"), "name = \"Friday Night Hub\"\nmotd = \"Gun game every Friday.\"\n").unwrap();
    let rocket = hub.invite_named("rocket");
    hub.redeem(&rocket, None).unwrap();
    let invited = hub.invite(InviteSpec { intent: Some("sent on Discord".into()), recipient: Some("Sam".into()), ..Default::default() });
    assert_eq!(invited.name, "Sam", "the recipient labels it");
    let views: Vec<chimera_services::accounts::InviteView> = hub.admin.get("/admin/invites").unwrap();
    let v = views.iter().find(|v| v.name == "Sam").unwrap();
    assert_eq!((v.intent.as_deref(), v.recipient.as_deref()), (Some("sent on Discord"), Some("Sam")));
    let e = hub.admin.post::<Invited>("/admin/invite", &InviteSpec::default()).err().unwrap();
    assert!(e.starts_with("/admin/invite: 400"), "no intent, no name: {e}");

    let answer = account::check(&hub.link(), &invited.code.to_lowercase(), Some("Rocket")).unwrap();
    assert_eq!(answer.kind, "invite");
    assert_eq!((answer.hub.name.as_str(), answer.hub.motd.as_str(), answer.hub.players), ("Friday Night Hub", "Gun game every Friday.", 1));
    let name = answer.username.unwrap();
    assert_eq!((name.username.as_str(), name.available, name.problem.as_deref()), ("rocket", false, Some("taken")));
    assert_eq!(name.suggestions, ["rocket2", "rocket_gg", "rocket_10"]);
    let bad = account::check(&hub.link(), &invited.code, Some("9 lives")).unwrap().username.unwrap();
    assert!(!bad.available && bad.problem.is_some() && bad.suggestions.is_empty());
    assert!(account::check(&hub.link(), &invited.code, Some("sam_rocket")).unwrap().username.unwrap().available);

    // Redeeming: a name must be chosen, and start with a letter.
    assert_eq!(status(&hub.redeem(&invited.code, None).unwrap_err()), 400);
    assert_eq!(status(&hub.redeem(&invited.code, Some("9lives")).unwrap_err()), 400);
    let request = RedeemRequest {
        code: invited.code.clone(),
        username: Some("Sam_Rocket".into()),
        avatar: Some(13),
        display_name: Some(" Sam Rocket ".into()),
        ..Default::default()
    };
    let (file, key) = account::redeem_request(&hub.link(), request).unwrap();
    assert!(key.is_some());
    assert_eq!(file.display_name, "Sam Rocket");
    let me = account::me(&file, file.ticket.as_deref().unwrap()).unwrap();
    assert_eq!((me["username"].as_str(), me["avatar"].as_u64()), (Some("sam_rocket"), Some(13)));
    let sam = hub.accounts().into_iter().find(|a| a.username == "sam_rocket").unwrap();
    assert_eq!(sam.invite_intent.as_deref(), Some("sent on Discord (for Sam)"));

    // Used, unknown and link codes as check sees them; wrong codes count.
    let e = account::check(&hub.link(), &invited.code, None).unwrap_err();
    assert_eq!(account::Failure::of(&e), account::Failure::Refused { status: 410, message: "this invite was used already".into() });
    let link = account::link_code(&file, file.ticket.as_deref().unwrap()).unwrap();
    let l = account::check(&hub.link(), &link.code, None).unwrap();
    assert_eq!((l.kind.as_str(), l.account.as_deref(), l.username), ("link", Some("sam_rocket"), None));
    for _ in 0..4 {
        assert_eq!(status(&account::check(&hub.link(), "AAA-AAA-AAA", None).unwrap_err()), 404);
    }
    assert_eq!(status(&account::check(&hub.link(), &link.code, None).unwrap_err()), 429, "the address's tries are used up");
}

/// Renames: a chosen name, at most one a minute, the last 10 kept as the
/// visible history; a taken name refused; the next ticket carries the
/// new name. An avatar is one of the 32 looks.
#[test]
fn renames_keep_a_visible_history() {
    let hub = Hub::new("rename");
    hub.redeem(&hub.invite_named("taken_name"), None).unwrap();
    let (file, _) = hub.redeem(&hub.invite_named("first"), None).unwrap();
    let t = file.ticket.clone().unwrap();
    assert_eq!(status(&account::rename(&file, &t, "taken_name").unwrap_err()), 409);
    assert_eq!(status(&account::rename(&file, &t, "1st").unwrap_err()), 400);
    let me = account::rename(&file, &t, "Second").unwrap();
    assert_eq!((me["username"].as_str(), me["previous_usernames"][0].as_str()), (Some("second"), Some("first")));
    assert_eq!(status(&account::rename(&file, &t, "third").unwrap_err()), 429, "one a minute");
    for i in 0..11 {
        hub.clock.advance(61);
        account::rename(&file, &t, &format!("name_{i}")).unwrap();
    }
    let view = hub.accounts().into_iter().find(|a| a.username == "name_10").unwrap();
    assert_eq!(view.previous_usernames.len(), 10);
    assert_eq!(view.previous_usernames[0], "name_9");
    assert!(!view.previous_usernames.contains(&"first".to_string()), "the oldest dropped");
    // Back to the real time: a device's request carries the PC's clock.
    hub.clock.advance(-(11 * 61));
    let fresh = account::request_ticket(&file).unwrap();
    let claims = hub.running.http.accounts.ticket_keys().check(&fresh, ticket::SERVICES, hub.clock.now()).unwrap();
    assert_eq!(claims.username, "name_10");
    assert_eq!(status(&account::set_avatar(&file, &t, 32).unwrap_err()), 400);
    assert_eq!(account::set_avatar(&file, &t, 31).unwrap()["avatar"].as_u64(), Some(31));
}

/// "Recover with key" on the onboarding's sign-in screen takes the key
/// alone: the account is the key's.
#[test]
fn a_recovery_key_alone_finds_its_account() {
    let hub = Hub::new("recover-key");
    let (old, key) = hub.redeem(&hub.invite_named("lena"), None).unwrap();
    let e = account::recover(&hub.link(), "", "0000-0000-0000-0000", "new pc").unwrap_err();
    assert_eq!(status(&e), 403, "{e}");
    let (new, again) = account::recover(&hub.link(), "", &key.unwrap(), "new pc").unwrap();
    assert_eq!((new.account.as_str(), new.username.as_str()), (old.account.as_str(), "lena"));
    assert!(again.is_some());
}

/// The account page's calls: a rename's availability with free names (the
/// sign-up's rule; its own name free to it), and signing this device out
/// (its tickets refused at once, the account's other devices kept).
#[test]
fn the_account_page_checks_renames_and_signs_out() {
    let hub = Hub::new("account-page");
    hub.redeem(&hub.invite_named("rocket"), None).unwrap();
    let (file, _) = hub.redeem(&hub.invite_named("sam"), None).unwrap();
    let t = file.ticket.clone().unwrap();
    let taken = account::check_rename(&file, &t, "Rocket").unwrap();
    assert_eq!((taken.available, taken.problem.as_deref()), (false, Some("taken")));
    assert_eq!(taken.suggestions, ["rocket2", "rocket_gg", "rocket_10"]);
    assert!(account::check_rename(&file, &t, "sam").unwrap().available, "its own name");
    assert!(account::check_rename(&file, &t, "sam_rocket").unwrap().available);
    assert!(!account::check_rename(&file, &t, "9lives").unwrap().available);
    let link = account::link_code(&file, &t).unwrap();
    let (other, _) = account::redeem(&hub.link(), &link.code, None, "other pc").unwrap();
    account::sign_out(&file, &t).unwrap();
    assert_eq!(status(&account::me(&file, &t).unwrap_err()), 401, "this device's ticket is refused");
    assert_eq!(status(&account::sign_out(&file, &t).unwrap_err()), 401);
    assert!(account::request_ticket(&file).is_err(), "no new tickets for it");
    assert!(account::me(&other, other.ticket.as_deref().unwrap()).is_ok(), "the other PC stays signed in");
}

/// An owner's invites in the game: made with an intent, a recipient and a
/// tester's or host's role (never an owner's), the link filled from the
/// account's hub; listed while open, newest first; revoked by id (a
/// revoked one can't be redeemed). Anyone else is refused.
#[test]
fn an_owner_makes_lists_and_revokes_invites_in_the_game() {
    use chimera_assets::account::InviteRequest;
    let hub = Hub::new("owner-invites");
    let (owner, _) = hub.redeem(&hub.invite(InviteSpec { name: "boss".into(), role: Some("owner".into()), ..Default::default() }).code, None).unwrap();
    let (player, _) = hub.redeem(&hub.invite_named("player"), None).unwrap();
    let (ot, pt) = (owner.ticket.clone().unwrap(), player.ticket.clone().unwrap());
    let ask = |intent: &str, recipient: Option<&str>, role: Option<&str>| InviteRequest {
        intent: intent.into(),
        recipient: recipient.map(String::from),
        role: role.map(String::from),
    };
    assert_eq!(status(&account::make_invite(&player, &pt, &ask("for Kim", None, None)).unwrap_err()), 403);
    assert_eq!(status(&account::open_invites(&player, &pt).unwrap_err()), 403);
    assert_eq!(status(&account::make_invite(&owner, &ot, &ask("  ", None, None)).unwrap_err()), 400, "an intent is needed");
    assert_eq!(status(&account::make_invite(&owner, &ot, &ask("x", None, Some("owner"))).unwrap_err()), 400, "no owner invites");

    let kim = account::make_invite(&owner, &ot, &ask("sent on Discord", Some("Kim"), None)).unwrap();
    let link = HubLink::parse(kim.link.as_deref().unwrap()).unwrap();
    assert_eq!((link.host.as_str(), link.code.as_deref()), (hub.link().host.as_str(), Some(kim.code.as_str())));
    let lee = account::make_invite(&owner, &ot, &ask("a LAN host", Some("Lee"), Some("host"))).unwrap();
    let open = account::open_invites(&owner, &ot).unwrap();
    let names: Vec<(&str, &str)> = open.iter().map(|i| (i.name.as_str(), i.role.as_str())).collect();
    assert!(names.starts_with(&[("Lee", "host"), ("Kim", "tester")]), "newest first: {names:?}");
    let kim_open = open.iter().find(|i| i.name == "Kim").unwrap();
    assert_eq!((kim_open.intent.as_deref(), kim_open.recipient.as_deref()), (Some("sent on Discord"), Some("Kim")));

    // Redeemed or revoked: no longer open; a revoked code is refused.
    let request = RedeemRequest { code: lee.code.clone(), username: Some("lee".into()), ..Default::default() };
    account::redeem_request(&hub.link(), request).unwrap();
    account::revoke_invite(&owner, &ot, kim_open.id).unwrap();
    assert_eq!(status(&account::revoke_invite(&owner, &ot, kim_open.id).unwrap_err()), 404, "once");
    assert!(account::open_invites(&owner, &ot).unwrap().iter().all(|i| i.name != "Kim" && i.name != "Lee"));
    assert_eq!(status(&account::check(&hub.link(), &kim.code, None).unwrap_err()), 410);
    let lee_account = hub.accounts().into_iter().find(|a| a.username == "lee").unwrap();
    assert_eq!(lee_account.role, "host");
    // A demoted owner is refused at once (the role read at the call).
    let _: AccountView = hub.admin.post("/admin/account/set", &serde_json::json!({ "username": "boss", "role": "tester" })).unwrap();
    assert_eq!(status(&account::open_invites(&owner, &ot).unwrap_err()), 403);
}
