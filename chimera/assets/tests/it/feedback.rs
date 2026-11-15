//! Feedback reports (docs/plans/active/feedback-reports.md, P1a): a bundle
//! built from a `ReportState` full of planted secrets holds none of them
//! and only allow-listed entries; the asset service's `POST /feedback` on
//! a free port refuses no token (401), too large (413), past a limit
//! (429), a `..` or unknown entry (400), stores the same id once, mutes,
//! and keeps statuses with their history; the page answers only this
//! machine's direct connections; the outbox retries a failing service
//! until it answers, and gives up on a refusal.

use std::{
    fs,
    net::IpAddr,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

use chimera_assets::{
    Remote, RemoteConfig, Store,
    accounts::Accounts,
    feedback::{
        CvarValue, Feedback, ImageFormat, Kind, OutboxConfig, OutboxEvent, ReportState, Screenshot, Target, Targets,
        bundle, inbox::{Change, FeedbackLimits, Inbox, Status}, new_report_id, outbox, zip,
    },
    server::{self, Running, Service},
    tls::{self, Identity, Peers},
};

use super::scratch;

/// Strings that must never reach a bundle.
pub(crate) const PLANTED: &[&str] = &[
    "PlantedRcon77",
    "PlantedSvPw",
    "chimera1_5e3c0ffee5e3c0ffee5e3c0ffee5e3c0ffee",
    "5e3c0ffee5e3c0ffee",
    "Zq9xK2mP7vL4tR8wN3bY6hJ1",
    "203.0.113.77",
    "198.51.100.23",
    "2001:db8:ab::17",
    "PlantUser",
    "plantuser",
    "game.example.net",
    "PlantedAuthCvar",
    "PlantedServerOnly",
    "aa11bb22cc33dd44ee55ff66aa11bb22cc33dd44ee55ff66aa11bb22cc33dd44",
];

fn planted_state() -> ReportState {
    let token = "chimera1_5e3c0ffee5e3c0ffee5e3c0ffee5e3c0ffee";
    ReportState {
        kind: Kind::Bug,
        text: "The gun vanished on rung 3.\nMy server was 203.0.113.77, token Zq9xK2mP7vL4tR8wN3bY6hJ1".into(),
        build: "test-build".into(),
        net_version: "net50".into(),
        app: "ultimate_gun_game".into(),
        install_id: "0123456789abcdef0123456789abcdef".into(),
        screenshot: Some(Screenshot { format: ImageFormat::Png, bytes: b"\x89PNG\r\n\x1a\nnot really a png".to_vec() }),
        game: "map: de_dust2\nserver: Friday night (198.51.100.23:27015)\nsetpos 1 2 3;setang 0 90 0\n".into(),
        system: concat!("OS: Windows 11\nexe: C:\\", "Users", "\\PlantUser\\Games\\ugg\\ugg.exe\n").into(),
        perf: "fps p50 144\n".into(),
        net: "ping 40 ms to [2001:db8:ab::17]:27015\n".into(),
        cvars: vec![
            CvarValue { name: "fps_max".into(), value: "200".into(), default: "300".into(), server_private: false },
            CvarValue { name: "rcon_password".into(), value: "PlantedRcon77".into(), default: "".into(), server_private: false },
            CvarValue { name: "sv_password".into(), value: "PlantedSvPw".into(), default: "".into(), server_private: false },
            CvarValue { name: "chimera:asset_token".into(), value: token.into(), default: "".into(), server_private: false },
            CvarValue {
                name: "chimera:feedback_fingerprint".into(),
                value: "aa11bb22cc33dd44ee55ff66aa11bb22cc33dd44ee55ff66aa11bb22cc33dd44".into(),
                default: "".into(),
                server_private: false,
            },
            CvarValue { name: "sv_authkey".into(), value: "PlantedAuthCvar".into(), default: "".into(), server_private: false },
            CvarValue { name: "bot_quota".into(), value: "PlantedServerOnly".into(), default: "0".into(), server_private: true },
        ],
        binds: vec![
            ("F10".into(), "feedback".into()),
            ("F1".into(), "rcon_password PlantedRcon77".into()),
        ],
        console: vec![
            "] fps_max 200".into(),
            "] rcon_password PlantedRcon77".into(),
            format!("] +chimera:asset_token {token}"),
            "\"sv_password\" = \"PlantedSvPw\" ( def. \"\" )".into(),
            "] connect game.example.net:27015".into(),
            "Connecting to 203.0.113.77:27015...".into(),
            concat!("loading /", "home", "/plantuser/.local/share/chimera/cfg/config.cfg").into(),
            "pinned aa11bb22cc33dd44ee55ff66aa11bb22cc33dd44ee55ff66aa11bb22cc33dd44".into(),
            "weapon_ak47 picked up".into(),
        ],
        log: vec![
            "INFO assets: bound the token to https://198.51.100.23:27031".into(),
            "WARN net: reconnect to 2001:db8:ab::17 (attempt 2)".into(),
            concat!("INFO ui: opened C:\\", "Users", "\\PlantUser\\AppData\\Local\\chimera").into(),
            "INFO map: de_dust2 loaded".into(),
        ],
        ..ReportState::default()
    }
}

/// Every entry's bytes as text.
fn entries_text(bytes: &[u8]) -> Vec<(String, String)> {
    let opened = bundle::open(bytes).expect("a valid bundle");
    opened.entries.iter().map(|e| (e.name.clone(), String::from_utf8_lossy(&e.data).into_owned())).collect()
}

#[test]
fn a_bundle_holds_no_planted_secret_and_only_allowed_entries() {
    let state = planted_state();
    let built = bundle::build(&state).unwrap();
    let entries = entries_text(&built.bytes);
    for (name, text) in &entries {
        assert!(bundle::cap(name).is_some(), "{name} isn't allow-listed");
        for secret in PLANTED {
            assert!(!text.contains(secret), "{name} holds {secret:?}:\n{text}");
        }
    }
    let get = |n: &str| entries.iter().find(|(name, _)| name == n).map(|(_, t)| t.clone()).unwrap_or_default();
    // What isn't secret is kept.
    assert!(get("cvars.txt").contains("fps_max \"200\""), "{}", get("cvars.txt"));
    assert!(get("cvars.txt").contains("bind F10 \"feedback\""));
    assert!(!get("cvars.txt").contains("bot_quota"), "server-private cvars are dropped");
    assert!(get("console.txt").contains("weapon_ak47 picked up"));
    assert!(get("console.txt").contains("connect (address)"));
    assert!(get("log.txt").contains("de_dust2 loaded"));
    assert!(get("report.json").contains("The gun vanished on rung 3."));
    assert!(get("game.txt").contains("map: de_dust2"));
    assert!(entries.iter().any(|(n, _)| n == "screenshot.png"));
    let info = bundle::open(&built.bytes).unwrap().info;
    assert_eq!((info.id.as_str(), info.kind), (built.id.as_str(), Kind::Bug));
}

#[test]
fn a_bundle_caps_text_lines_and_its_size() {
    let mut state = planted_state();
    state.text = "x".repeat(5000);
    state.console = (0..1000).map(|i| format!("line {i}")).collect();
    // A screenshot past the cap is left out, and the report says so.
    state.screenshot = Some(Screenshot { format: ImageFormat::Jpeg, bytes: [&[0xFF, 0xD8, 0xFF][..], &vec![7u8; 9 << 20]].concat() });
    let built = bundle::build(&state).unwrap();
    assert!(built.screenshot_dropped);
    assert!(built.bytes.len() <= chimera_assets::feedback::MAX_BUNDLE);
    let opened = bundle::open(&built.bytes).unwrap();
    assert_eq!(opened.info.text.chars().count(), chimera_assets::feedback::MAX_TEXT_CHARS);
    assert_eq!(opened.info.screenshot_dropped.as_deref(), Some("too large"));
    let console = &opened.entries.iter().find(|e| e.name == "console.txt").unwrap().data;
    let console = String::from_utf8_lossy(console);
    assert_eq!(console.lines().count(), bundle::CONSOLE_LINES);
    assert!(console.starts_with("line 700\n") && console.ends_with("line 999\n"));
}

struct Receiver {
    store: Store,
    running: Running,
}

impl Receiver {
    fn new(name: &str, limits: FeedbackLimits) -> Self {
        let store = Store::new(scratch(name).join("server"));
        let service = Service::new(store.clone()).unwrap().with_feedback(limits, None);
        let running = server::serve(service, "127.0.0.1:0", 2).unwrap();
        Self { store, running }
    }

    fn user(&self, name: &str) -> String {
        let path = Accounts::path(&self.store.root);
        let mut accounts = Accounts::load(&path).unwrap();
        let (_, token) = accounts.add(name, None, "test").unwrap();
        accounts.save(&path).unwrap();
        token
    }

    fn remote(&self, token: &str) -> Remote {
        Remote::new(RemoteConfig { url: self.running.url(), token: token.into(), key: None, fingerprint: None }).unwrap()
    }

    fn inbox(&self) -> Inbox {
        Inbox::open(server::feedback_dir(&self.store))
    }
}

fn post(remote: &Remote, id: &str, body: &[u8]) -> (u16, Option<String>, String) {
    remote.post_raw("/feedback", "application/zip", &[("X-Chimera-Report-Id", id)], body).unwrap()
}

/// A valid bundle with a new id.
fn report(text: &str) -> (String, Vec<u8>) {
    let b = bundle::build(&ReportState { text: text.into(), ..ReportState::default() }).unwrap();
    (b.id, b.bytes)
}

#[test]
fn the_receiver_checks_tokens_sizes_entries_and_stores_once() {
    let limits = FeedbackLimits { max_body: 64 * 1024, ..FeedbackLimits::default() };
    let r = Receiver::new("feedback_receiver", limits);
    let token = r.user("tester");
    let (id, bytes) = report("first report\nsecond line");
    // No token, a wrong one.
    assert_eq!(post(&r.remote(""), &id, &bytes).0, 401);
    assert_eq!(post(&r.remote("chimera1_wrong"), &id, &bytes).0, 401);
    let remote = r.remote(&token);
    // Stored once; the same id again is 200.
    assert_eq!(post(&remote, &id, &bytes).0, 201);
    assert_eq!(post(&remote, &id, &bytes).0, 200);
    let inbox = r.inbox();
    let meta = inbox.meta(&id).unwrap();
    assert_eq!((meta.status, meta.summary.as_str(), meta.kind.as_str()), (Status::New, "first report", "bug"));
    assert!(meta.account.starts_with("internal:tester"));
    let dir = inbox.find(&id).unwrap();
    assert!(dir.join("report.zip").is_file() && dir.join("report.json").is_file());
    assert!(inbox.history(&id).contains("received from internal:tester"));
    assert_eq!(inbox.list().len(), 1);
    // Too large.
    let big = bundle::build(&ReportState {
        screenshot: Some(Screenshot { format: ImageFormat::Png, bytes: [&b"\x89PNG\r\n\x1a\n"[..], &(0..200_000u32).map(|i| (i * 7919 % 251) as u8).collect::<Vec<_>>()].concat() }),
        ..ReportState::default()
    })
    .unwrap();
    assert_eq!(post(&remote, &big.id, &big.bytes).0, 413);
    // A header id that isn't the bundle's, or none.
    assert_eq!(post(&remote, &new_report_id(), &bytes).0, 400);
    assert_eq!(remote.post_raw("/feedback", "application/zip", &[], &bytes).unwrap().0, 400);
    // A `..` entry and an unknown entry are refused, not stored.
    for bad in ["../evil.txt", "notes.txt", "sub/game.txt"] {
        let id = new_report_id();
        let mut w = zip::ZipWriter::new(0);
        let json = format!(r#"{{"schema":1,"id":"{id}","kind":"bug","text":"x","time":0}}"#);
        w.add("report.json", json.as_bytes(), false);
        w.add(bad, b"hello", false);
        let (status, _, body) = post(&remote, &id, &w.finish());
        assert_eq!(status, 400, "{bad}: {body}");
        assert!(inbox.find(&id).is_none());
    }
    // Not a zip at all.
    assert_eq!(post(&remote, &new_report_id(), b"hello").0, 400);
    assert_eq!(inbox.list().len(), 1);
    assert!(!fs::read_dir(server::feedback_dir(&r.store)).unwrap().flatten().any(|e| e.file_name() == ".."));
}

#[test]
fn the_receiver_limits_accounts_and_addresses_and_mutes() {
    let limits = FeedbackLimits { per_account_hour: 2, per_account_day: 40, per_address_hour: 3, ..FeedbackLimits::default() };
    let r = Receiver::new("feedback_limits", limits);
    let (a, b) = (r.remote(&r.user("alpha")), r.remote(&r.user("beta")));
    for _ in 0..2 {
        let (id, bytes) = report("ok");
        assert_eq!(post(&a, &id, &bytes).0, 201);
    }
    let (id, bytes) = report("one too many");
    let (status, retry, _) = post(&a, &id, &bytes);
    assert_eq!(status, 429);
    let retry: u64 = retry.expect("Retry-After").parse().unwrap();
    assert!((1..=3600).contains(&retry), "{retry}");
    // Another account from the same address: one more, then the
    // address's limit.
    let (id, bytes) = report("beta's");
    assert_eq!(post(&b, &id, &bytes).0, 201);
    let (id, bytes) = report("beta's second");
    assert_eq!(post(&b, &id, &bytes).0, 429);
    // Muted: 202 and nothing stored (the inbox's own limits are fresh).
    let inbox = r.inbox().with_limits(FeedbackLimits::default());
    inbox.set_muted("alpha", true).unwrap();
    let (id, bytes) = report("troll");
    let ip: IpAddr = "127.0.0.1".parse().unwrap();
    assert_eq!(inbox.receive("internal:alpha#1", Some(ip), Some(&id), &bytes).status, 202);
    assert!(inbox.find(&id).is_none());
    inbox.set_muted("alpha", false).unwrap();
    assert_eq!(inbox.receive("internal:alpha#1", Some(ip), Some(&id), &bytes).status, 201);
}

#[test]
fn statuses_keep_a_history_and_the_page_is_for_this_machine_only() {
    let r = Receiver::new("feedback_status", FeedbackLimits::default());
    let remote = r.remote(&r.user("tester"));
    let (id, bytes) = report("the menu froze");
    assert_eq!(post(&remote, &id, &bytes).0, 201);
    let inbox = r.inbox();
    assert!(inbox.set_status(&id, "cli", &Change { status: Some(Status::Duplicate), ..Change::default() }).is_err());
    let meta = inbox
        .set_status(&id, "cli", &Change { status: Some(Status::Accepted), reason: Some("real".into()), ..Change::default() })
        .unwrap();
    assert_eq!(meta.status, Status::Accepted);
    assert!(inbox.history(&id).contains("cli new -> accepted reason=\"real\""), "{}", inbox.history(&id));

    // The page over plain HTTP from this machine.
    let base = r.running.url();
    let agent = ureq::Agent::config_builder().http_status_as_error(false).build().new_agent();
    let get = |path: &str, host: Option<&str>| {
        let mut req = agent.get(&format!("{base}{path}"));
        if let Some(h) = host {
            req = req.header("Host", h);
        }
        let mut resp = req.call().unwrap();
        (resp.status().as_u16(), resp.body_mut().read_to_string().unwrap_or_default())
    };
    let (status, html) = get("/feedback/admin/", None);
    assert_eq!(status, 200);
    assert!(html.contains("Feedback reports"));
    let (status, list) = get("/feedback/admin/list", None);
    assert_eq!(status, 200);
    assert!(list.contains(&id) && list.contains("accepted"));
    assert_eq!(get(&format!("/feedback/admin/{id}/report.json"), None).0, 200);
    assert_eq!(get(&format!("/feedback/admin/{id}/..%2Fmeta.toml"), None).0, 404);
    assert_eq!(get("/feedback/admin/list", Some("evil.example")).0, 404, "a foreign Host (DNS rebinding)");
    let post_status = |admin: bool| {
        let mut req = agent.post(&format!("{base}/feedback/admin/{id}/status")).header("Content-Type", "application/json");
        if admin {
            req = req.header("X-Chimera-Admin", "1");
        }
        req.send(r#"{"status":"fixed","fixed_in":"abc1234"}"#).unwrap().status().as_u16()
    };
    assert_eq!(post_status(false), 403);
    assert_eq!(post_status(true), 200);
    assert_eq!(inbox.meta(&id).unwrap().fixed_in, "abc1234");
    assert!(inbox.history(&id).contains("page accepted -> fixed"));

    // Through the TLS front (as every tester comes): no page.
    let identity = Identity::from_seed(&[9; 32], [1; 16]);
    let peers = Peers::default();
    let service = Service::new(r.store.clone()).unwrap().with_peers(peers.clone());
    let plain = server::serve(service, "127.0.0.1:0", 2).unwrap();
    let front = tls::front("127.0.0.1:0", plain.addr, &identity, peers, tls::FrontLimits::default()).unwrap();
    let pinned = Remote::new(RemoteConfig {
        url: front.url(),
        token: "none".into(),
        key: None,
        fingerprint: Some(identity.fingerprint()),
    })
    .unwrap();
    let (status, ..) = pinned
        .post_raw(&format!("/feedback/admin/{id}/status"), "application/json", &[("X-Chimera-Admin", "1")], b"{}")
        .unwrap();
    assert_eq!(status, 404);
}

/// A service that answers `statuses` in turn (then the last), counting
/// requests.
fn scripted(statuses: Vec<u16>) -> (String, Arc<AtomicUsize>, std::thread::JoinHandle<()>, Arc<tiny_http::Server>) {
    let server = Arc::new(tiny_http::Server::http("127.0.0.1:0").unwrap());
    let url = format!("http://{}", server.server_addr().to_ip().unwrap());
    let count = Arc::new(AtomicUsize::new(0));
    let (s, c) = (server.clone(), count.clone());
    let thread = std::thread::spawn(move || {
        while let Ok(mut req) = s.recv() {
            let mut body = Vec::new();
            let _ = req.as_reader().read_to_end(&mut body);
            let n = c.fetch_add(1, Ordering::SeqCst);
            let status = statuses[n.min(statuses.len() - 1)];
            let _ = req.respond(tiny_http::Response::from_string("scripted").with_status_code(status));
        }
    });
    (url, count, thread, server)
}

fn wait_for(feedback: &Feedback, want: impl Fn(&OutboxEvent) -> bool) -> Vec<OutboxEvent> {
    let start = Instant::now();
    let mut seen = Vec::new();
    while start.elapsed() < Duration::from_secs(20) {
        seen.extend(feedback.events());
        if seen.iter().any(&want) {
            return seen;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("not seen; events: {seen:?}");
}

fn fast() -> OutboxConfig {
    OutboxConfig { first_retry: Duration::from_millis(50), max_retry: Duration::from_millis(400), keep_sent: 20 }
}

fn target(url: &str) -> Targets {
    Targets { local: Some(Target { url: url.into(), token: "chimera1_test".into(), fingerprint: None }), ..Targets::default() }
}

#[test]
fn the_outbox_retries_until_the_service_answers_and_gives_up_on_a_refusal() {
    let (url, count, _t, server) = scripted(vec![503, 503, 201]);
    let folder = scratch("feedback_outbox");
    let feedback = Feedback::start(folder.clone(), target(&url), fast()).unwrap();
    let submitted = feedback.submit(&planted_state()).unwrap();
    assert!(submitted.path.is_file());
    assert!(submitted.destination.is_some());
    let events = wait_for(&feedback, |e| matches!(e, OutboxEvent::Sent { .. }));
    assert_eq!(count.load(Ordering::SeqCst), 3, "{events:?}");
    assert_eq!(events.iter().filter(|e| matches!(e, OutboxEvent::Queued { .. })).count(), 2);
    assert!(!submitted.path.exists());
    assert!(folder.join("sent").join(format!("{}.zip", submitted.id)).is_file());
    assert_eq!(feedback.status().pending(), 0);
    server.unblock();

    // A 400 is final: the zip stays, marked failed, not tried again.
    let (url, count, _t, server) = scripted(vec![400]);
    feedback.set_targets(|t| *t = target(&url));
    let submitted = feedback.submit(&planted_state()).unwrap();
    wait_for(&feedback, |e| matches!(e, OutboxEvent::Failed { .. }));
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(count.load(Ordering::SeqCst), 1);
    assert!(submitted.path.is_file());
    let status = feedback.status();
    assert!(status.waiting.iter().any(|w| w.failed && w.last_error.as_deref().is_some_and(|e| e.contains("400"))));
    server.unblock();
}

#[test]
fn without_a_destination_reports_wait_and_go_when_one_appears() {
    let r = Receiver::new("feedback_e2e", FeedbackLimits::default());
    let token = r.user("tester");
    let folder = scratch("feedback_e2e_client");
    let feedback = Feedback::start(folder, Targets::default(), fast()).unwrap();
    let submitted = feedback.submit(&planted_state()).unwrap();
    assert!(submitted.destination.is_none());
    wait_for(&feedback, |e| matches!(e, OutboxEvent::Queued { reason, .. } if reason.contains("no report service")));
    assert_eq!(feedback.status().pending(), 1);
    // A developer's asset service appears (end to end, the real receiver).
    feedback.set_targets(|t| {
        t.assets = Some(RemoteConfig { url: r.running.url(), token: token.clone(), key: None, fingerprint: None })
    });
    wait_for(&feedback, |e| matches!(e, OutboxEvent::Sent { .. }));
    let meta = r.inbox().meta(&submitted.id).expect("stored");
    assert_eq!(meta.map, "de_dust2");
    assert_eq!(meta.status, Status::New);
}

#[test]
fn the_destination_follows_the_plans_order() {
    use chimera_assets::access::Access;
    let (a, b) = ("aa".repeat(32), "bb".repeat(32));
    let mut access = Access::default();
    access.set_token(&format!("chimera1_{}", "0".repeat(64))).unwrap();
    access.bind("https://bound:1", &a).unwrap();
    let mut t = Targets { access: Some(access), offer: Some(("https://server:2".into(), a.to_uppercase())), server_name: Some("Friday".into()), ..Targets::default() };
    let d = outbox::resolve(&t).unwrap();
    assert_eq!((d.source, d.remote.url.as_str(), d.label.as_str()), (outbox::Source::Server, "https://server:2", "Friday"));
    // Another certificate: never the token to it; the bound service instead.
    t.offer = Some(("https://evil:3".into(), b));
    let d = outbox::resolve(&t).unwrap();
    assert_eq!((d.source, d.remote.url.as_str()), (outbox::Source::BoundService, "https://bound:1"));
    t.local = Some(Target { url: "https://local:4".into(), token: "x".repeat(10), fingerprint: None });
    assert_eq!(outbox::resolve(&t).unwrap().source, outbox::Source::LocalConfig);
    t.cvar = Some(Target { url: "https://cvar:5".into(), token: "y".repeat(10), fingerprint: None });
    assert_eq!(outbox::resolve(&t).unwrap().source, outbox::Source::Cvar);
    assert!(outbox::resolve(&Targets::default()).is_none());
    // A token bound to no service yet (a tester with their own install):
    // the offered one.
    let mut fresh = Access::default();
    fresh.set_token(&format!("chimera1_{}", "1".repeat(64))).unwrap();
    let t = Targets { access: Some(fresh), offer: Some(("https://server:2".into(), a.clone())), ..Targets::default() };
    let d = outbox::resolve(&t).unwrap();
    assert_eq!((d.source, d.remote.fingerprint.as_deref()), (outbox::Source::Server, Some(a.as_str())));
}

#[cfg(unix)]
#[test]
fn on_report_runs_the_hosts_command_with_a_cleaned_summary() {
    let dir = scratch("feedback_on_report");
    let out = dir.join("ping.txt");
    let inbox = Inbox::open(dir.join("feedback")).with_on_report(Some(format!(
        "printf '%s|%s|%s' \"$CHIMERA_REPORT_ID\" \"$CHIMERA_REPORT_ACCOUNT\" \"$CHIMERA_REPORT_SUMMARY\" > '{}'",
        out.display()
    )));
    let (id, bytes) = report("@everyone `rm -rf` <b>broken</b>");
    assert_eq!(inbox.receive("internal:tester#2", None, Some(&id), &bytes).status, 201);
    let start = Instant::now();
    while !out.is_file() || fs::read_to_string(&out).unwrap().is_empty() {
        assert!(start.elapsed() < Duration::from_secs(10), "the command didn't run");
        std::thread::sleep(Duration::from_millis(20));
    }
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(fs::read_to_string(&out).unwrap(), format!("{id}|internal:tester|everyone rm -rf bbroken/b"));
}

#[test]
fn the_route_takes_crash_reports_and_groups_them_by_signature() {
    let r = Receiver::new("feedback_crash", FeedbackLimits::default());
    let remote = r.remote(&r.user("tester"));
    let crash = |frame: &str| {
        let b = bundle::build(&ReportState {
            kind: Kind::Crash,
            text: "panic in thread 'main': boom at chimera/x.rs:3".into(),
            backtrace: format!(
                "panic in thread 'main': boom at chimera/x.rs:3\n\n   0: std::panicking::begin_panic\n   1: {frame}::h0123456789abcdef\n   2: chimera_client::client::app::run\n"
            ),
            health: vec!["12:00:00.000 health rss=812M".into()],
            minidump: Some(b"MDMP fake dump".to_vec()),
            log: vec!["INFO map: de_dust2 loaded".into()],
            ..ReportState::default()
        })
        .unwrap();
        (b.id, b.bytes)
    };
    let (a, a_bytes) = crash("chimera_client::client::hud::draw");
    let (b, b_bytes) = crash("chimera_client::client::hud::draw");
    let (c, c_bytes) = crash("chimera_world::map::spawn");
    for (id, bytes) in [(&a, &a_bytes), (&b, &b_bytes), (&c, &c_bytes)] {
        assert_eq!(post(&remote, id, bytes).0, 201);
    }
    let inbox = r.inbox();
    let meta = inbox.meta(&a).unwrap();
    assert_eq!(meta.kind, "crash");
    assert!(meta.signature.starts_with("panic chimera_client::client::hud::draw"), "{}", meta.signature);
    let dir = inbox.find(&a).unwrap();
    for entry in ["backtrace.txt", "health.txt", "minidump.dmp", "log.txt"] {
        assert!(dir.join(entry).is_file(), "{entry}");
    }
    let groups = inbox.crash_groups();
    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0].2.len(), 2);
    assert_eq!(groups[0].0, meta.signature_key);
    // The page: the groups, the minidump as bytes.
    let page = chimera_assets::feedback::page::handle(&inbox, false, &["crashes"], b"", false);
    let v: serde_json::Value = serde_json::from_slice(&page.body).unwrap();
    assert_eq!(v[0]["count"], 2);
    let dump = chimera_assets::feedback::page::handle(&inbox, false, &[&a, "minidump.dmp"], b"", false);
    assert_eq!((dump.status, dump.content_type), (200, "application/octet-stream"));
    // Not a minidump: refused.
    let mut w = zip::ZipWriter::new(0);
    let id = new_report_id();
    w.add("report.json", format!(r#"{{"schema":1,"id":"{id}","kind":"crash","text":"x","time":0}}"#).as_bytes(), false);
    w.add("minidump.dmp", b"not one", false);
    assert_eq!(post(&remote, &id, &w.finish()).0, 400);
    let _ = c;
}
