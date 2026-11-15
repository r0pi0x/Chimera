//! The accounts service with the asset service beside it, both on free
//! loopback ports over one scratch server store (as on the host): the
//! owner's invites through the admin API, a player's redeem, link codes,
//! recovery and tickets through the client side (`chimera_assets::account`),
//! downloads with tickets, revocation through the feed, legacy tokens.

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
    assert!(account::request_ticket(&wrong).is_err());
    // A legacy-shaped config with the live session's ticket: `Remote` sends a current one.
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
    // Display names: at most 20 characters, set with a ticket.
    let t = first.ticket.clone().unwrap();
    let set = |name: &str| -> Result<serde_json::Value, String> {
        let agent = chimera_assets::remote::agent(None).unwrap();
        let mut r = agent
            .post(&format!("{}/accounts/display-name", hub.running.url()))
            .header("Authorization", &format!("Bearer {t}"))
            .header("Content-Type", "application/json")
            .send(serde_json::json!({ "display_name": name }).to_string().as_bytes())
            .map_err(|e| e.to_string())?;
        let code = r.status().as_u16();
        let body = r.body_mut().read_to_string().unwrap_or_default();
        if code == 200 { Ok(serde_json::from_str(&body).unwrap()) } else { Err(format!("{code} {body}")) }
    };
    assert!(set(&"z".repeat(21)).unwrap_err().starts_with("400"));
    assert_eq!(set("  The Mixed One  ").unwrap()["display_name"], "The Mixed One");
    let mine = hub.accounts().into_iter().find(|a| a.username == "mixed_case").unwrap();
    assert_eq!(mine.display_name, "The Mixed One");
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
    };
    let signed = hub.running.http.accounts.redeem(&request, None).unwrap();
    let device = ticket::parse_id(&signed.device).unwrap();
    let ask = |time: u64, nonce: [u8; 16]| {
        let signature = key.sign(&ticket::request_message(&device, time, &nonce));
        let r = TicketRequest { device: signed.device.clone(), time, nonce: hex(&nonce), signature: hex(&signature.to_bytes()) };
        hub.running.http.accounts.ticket(Some(&r), None, None)
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
    assert_eq!(hub.running.http.accounts.ticket(Some(&r), None, None).unwrap_err().status, 401);
}

/// A legacy `chimera1_` token keeps downloading; after import-legacy it
/// also gets tickets (its legacy device), and importing again skips it.
#[test]
fn a_legacy_token_still_works() {
    let hub = Hub::new("legacy");
    let path = chimera_assets::accounts::Accounts::path(&hub.paths.store);
    let mut legacy = chimera_assets::accounts::Accounts::default();
    let (_, token) = legacy.add("Old.Tester", Some(vec!["fixture".into()]), "laptop").unwrap();
    legacy.save(&path).unwrap();
    assert!(hub.remote(&token).whoami().unwrap().starts_with("internal Old.Tester 1"));
    assert_eq!(status(&account::legacy_ticket(&hub.running.url(), None, &token).unwrap_err()), 401, "not imported yet");
    let text = fs::read_to_string(&path).unwrap();
    let done: chimera_services::accounts::Imported =
        hub.admin.post("/admin/import-legacy", &serde_json::json!({ "toml": text })).unwrap();
    assert_eq!((done.accounts, done.devices, done.skipped), (1, 1, 0));
    let ticket = account::legacy_ticket(&hub.running.url(), None, &token).unwrap();
    let remote = hub.remote(&ticket);
    assert!(remote.whoami().unwrap().starts_with("account old_tester 0\ngames fixture"));
    assert!(remote.record(&hub.records.0).is_ok());
    assert!(hub.remote(&token).whoami().is_ok(), "the token itself still works");
    let again: chimera_services::accounts::Imported =
        hub.admin.post("/admin/import-legacy", &serde_json::json!({ "toml": text })).unwrap();
    assert_eq!((again.accounts, again.skipped), (0, 1));
    let view = hub.accounts().into_iter().find(|a| a.username == "old_tester").unwrap();
    assert_eq!((view.display_name.as_str(), view.devices[0].kind.as_str()), ("Old.Tester", "legacy"));
}

/// An import carries `accounts.toml`'s revocations over, also to accounts
/// imported before, and never re-enables what the database revoked; a
/// revocation made here reaches the file, so the asset service refuses the
/// token itself too.
#[test]
fn an_import_carries_revocations_and_never_reenables() {
    use chimera_assets::accounts::Accounts as Legacy;
    let hub = Hub::new("legacy-revoked");
    let path = Legacy::path(&hub.paths.store);
    let mut legacy = Legacy::default();
    let (_, gone) = legacy.add("gone", None, "pc").unwrap();
    let (_, late) = legacy.add("late", None, "pc").unwrap();
    let (_, kept) = legacy.add("kept", None, "pc").unwrap();
    legacy.revoke("gone", None).unwrap();
    legacy.save(&path).unwrap();
    let import = |text: &str| -> serde_json::Value {
        hub.admin.post("/admin/import-legacy", &serde_json::json!({ "toml": text })).unwrap()
    };
    let count = |v: &serde_json::Value, k: &str| v[k].as_u64().unwrap_or(0);
    let file = || fs::read_to_string(&path).unwrap();
    let state = |name: &str| hub.accounts().into_iter().find(|a| a.username == name).unwrap().state;
    let ticket_status = |token: &str| account::legacy_ticket(&hub.running.url(), None, token).err().map(|e| status(&e));

    // A token revoked before the import: its account comes in revoked.
    let done = import(&file());
    assert_eq!(count(&done, "accounts"), 3);
    assert_eq!(state("gone"), "revoked");
    assert_eq!(ticket_status(&gone), Some(401));
    assert!(hub.remote(&gone).whoami().is_err());
    assert_eq!(ticket_status(&late), None, "a live token gets tickets");

    // Revoked in the file after the import: a re-import carries it over.
    let mut legacy = Legacy::load(&path).unwrap();
    legacy.revoke("late", None).unwrap();
    legacy.save(&path).unwrap();
    let again = import(&file());
    assert_eq!((count(&again, "accounts"), count(&again, "skipped"), count(&again, "revoked")), (0, 3, 1));
    assert_eq!(ticket_status(&late), Some(401), "no more tickets for a token the file revoked");
    assert_eq!(state("late"), "revoked");

    // Revoked here: the file follows, and importing the old file (the
    // token still live in it) doesn't bring the account back.
    let before = file();
    assert!(hub.remote(&kept).whoami().is_ok());
    hub.admin.post::<()>("/admin/account/revoke", &serde_json::json!({ "username": "kept" })).unwrap();
    assert!(hub.remote(&kept).whoami().is_err(), "the asset service refuses the token itself");
    let old = import(&before);
    assert_eq!((count(&old, "accounts"), count(&old, "revoked")), (0, 0));
    assert_eq!(state("kept"), "revoked");
    assert_eq!(ticket_status(&kept), Some(401));

    // A deleted account's token is revoked in the file, so an import
    // brings it back revoked, never live.
    hub.admin.post::<()>("/admin/account/delete", &serde_json::json!({ "username": "kept" })).unwrap();
    let after = import(&file());
    assert_eq!(count(&after, "accounts"), 1);
    assert_eq!(state("kept"), "revoked");
    assert_eq!(ticket_status(&kept), Some(401));
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
