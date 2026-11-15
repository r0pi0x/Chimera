//! A session that died, at the next start (`client::crash`,
//! docs/plans/active/feedback-reports.md section 13): with
//! `cl_crash_reports ask` (the default) the notice is offered once
//! config.cfg has run and Send queues the report and wakes the outbox;
//! `never` settles it unsent; `always` (the notice's Always send) sends
//! without asking. The cvar sets the crash hooks' policy.

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use bevy::prelude::*;
use chimera_client::client::{
    crash::{self, CrashAction, CrashPlugin, CrashReports, CrashSetting},
    feedback::{FeedbackOutbox, Outbox, Report, Submitted},
};
use chimera_core::{
    console::{Console, ConsolePlugin},
    diagnostics::{CrashPolicy, Session, SessionOptions, session},
};

/// An outbox that counts its wakes.
struct Woken(Arc<AtomicUsize>);

impl Outbox for Woken {
    fn destination(&self) -> String {
        "test".into()
    }
    fn submit(&self, _: Report) -> Result<Submitted, String> {
        Err("not here".into())
    }
    fn wake(&self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

/// A folder with one session that died, and an app at its next start.
fn restart(name: &str, setting: Option<&str>) -> (App, std::path::PathBuf, Arc<AtomicUsize>) {
    let root = std::env::temp_dir().join(format!("chimera_client_crash_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let options = || SessionOptions {
        root: root.clone(),
        prefix: "session".into(),
        app: "ultimate_gun_game".into(),
        build: "test".into(),
        net_version: "net".into(),
    };
    let (dead, _) = Session::start(options()).unwrap();
    dead.line("INFO map: loaded");
    dead.health("health rss=7.9G avail=90M/16G");
    dead.abandon();
    drop(dead);
    let (now, previous) = Session::start(options()).unwrap();
    assert_eq!(previous.len(), 1);
    let woken = Arc::new(AtomicUsize::new(0));
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, ConsolePlugin, CrashPlugin))
        .insert_resource(FeedbackOutbox(Arc::new(Woken(woken.clone()))))
        .insert_resource(chimera_core::diagnostics::SessionLog(now));
    crash::found(&mut app, previous);
    if let Some(s) = setting {
        app.world_mut().resource_mut::<Console>().submit(format!("cl_crash_reports {s}"));
    }
    (app, root, woken)
}

fn outbox_zips(root: &std::path::Path) -> usize {
    std::fs::read_dir(root.join("feedback").join("outbox"))
        .map(|d| d.flatten().filter(|e| e.file_name().to_string_lossy().ends_with(".zip")).count())
        .unwrap_or(0)
}

#[test]
fn ask_offers_the_notice_and_send_queues_the_report() {
    let (mut app, root, woken) = restart("ask", None);
    app.update();
    assert!(!app.world().resource::<CrashReports>().offer, "not before config.cfg has run");
    for _ in 0..6 {
        app.update();
    }
    let r = app.world().resource::<CrashReports>();
    assert!(r.offer && r.decided);
    assert_eq!(r.headline(), "The last session ended unexpectedly. Send a report?");
    app.world_mut().write_message(CrashAction::Send);
    app.update();
    let r = app.world().resource::<CrashReports>();
    assert!(!r.offer && r.previous.is_empty());
    assert!(r.said.as_deref().is_some_and(|s| s.starts_with("Crash report queued")));
    assert_eq!(outbox_zips(&root), 1);
    assert_eq!(woken.load(Ordering::SeqCst), 1);
}

#[test]
fn never_settles_it_and_always_sends_without_asking() {
    let (mut app, root, _) = restart("never", Some("never"));
    for _ in 0..8 {
        app.update();
    }
    assert!(!app.world().resource::<CrashReports>().offer);
    assert_eq!(outbox_zips(&root), 0);
    assert!(app.world().resource::<CrashReports>().previous.is_empty());
    assert_eq!(app.world().resource::<CrashSetting>().policy(), CrashPolicy::Never);

    let (mut app, root, woken) = restart("always", Some("always"));
    for _ in 0..8 {
        app.update();
    }
    assert!(!app.world().resource::<CrashReports>().offer);
    assert_eq!(outbox_zips(&root), 1);
    assert_eq!(woken.load(Ordering::SeqCst), 1);
    assert_eq!(app.world().resource::<CrashSetting>().policy(), CrashPolicy::Always);
    session::set_policy(CrashPolicy::Ask);
}

#[test]
fn the_notice_s_always_send_sets_the_cvar() {
    let (mut app, root, _) = restart("always_button", None);
    for _ in 0..6 {
        app.update();
    }
    assert!(app.world().resource::<CrashReports>().offer);
    app.world_mut().write_message(CrashAction::Always);
    for _ in 0..3 {
        app.update();
    }
    assert_eq!(outbox_zips(&root), 1);
    let cvar = app.world().resource::<Console>().cvar("chimera:cl_crash_reports").cloned().expect("the cvar");
    assert!(cvar.archive, "saved in config.cfg");
    assert_eq!(app.world().resource::<CrashSetting>().0, "always");
    session::set_policy(CrashPolicy::Ask);
}
