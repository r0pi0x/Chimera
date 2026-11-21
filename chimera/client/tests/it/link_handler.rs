//! Invite links that open the game (`client::link_handler`): only a
//! well-formed invite passes, a start by a link takes nothing else, the
//! registration's writes (Windows' `reg.exe` calls through a fake, Linux's
//! files under temporary folders: never the real desktop's), and the inbox
//! that hands a link to the game already running.

use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use chimera_client::client::link_handler::{
    Handler, LinkHandlerPlugin, LinkInbox, LinkSetup, OpenedLinks, RegExe, SchemeRegistry, Xdg, accept, default_in, desktop_exec,
    ensure, link_launch, parse_reg_query, reg_query, with_default,
};
use chimera_sim::SimBasePlugins;
use chimera_world::harness::Sim;

fn fp() -> String {
    "3f9a0b71c2d45e6f".repeat(4)
}

fn link() -> String {
    format!("chimera-hub://hub.example.net:27080/{}?code=K7Q-M2P-9TR", fp())
}

/// A fresh folder of this test's own, under the system's temporary one.
fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("chimera-link-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn only_a_well_formed_invite_link_passes() {
    assert_eq!(accept(&link()).as_deref(), Some(link().as_str()));
    // The scheme in any case, a link code, no fingerprint, an IP, a
    // trailing slash (as some browsers add one).
    assert!(accept(&link().replace("chimera-hub", "CHIMERA-HUB")).is_some());
    assert!(accept("chimera-hub://10.0.0.5:27080?code=ABC-123").is_some());
    assert!(accept(&format!("chimera-hub://[::1]:27080/{}/?code=K7Q-M2P-9TR", fp())).is_some());
    for bad in [
        String::new(),
        "https://hub.example.net/invite/K7Q-M2P-9TR".to_string(),
        "chimera-hub://hub.example.net:27080".to_string(),
        "chimera-hub://hub.example.net/?code=K7Q-M2P-9TR".to_string(),
        "chimera-hub://hub.example.net:99999?code=K7Q-M2P-9TR".to_string(),
        "chimera-hub://hub.example.net:27080?code=K7Q".to_string(),
        format!("{}&exec=quit", link()),
        format!("{}#x", link()),
        format!("{} +exec evil.cfg", link()),
        format!("{}\" +quit \"", link()),
        "chimera-hub://user@hub.example.net:27080?code=K7Q-M2P-9TR".to_string(),
        "chimera-hub://hub;rm:27080?code=K7Q-M2P-9TR".to_string(),
        "chimera-hub://hub.example.net:27080/nothex?code=K7Q-M2P-9TR".to_string(),
        format!("chimera-hub://{}:27080?code=K7Q-M2P-9TR", "a".repeat(300)),
    ] {
        assert_eq!(accept(&bad), None, "{bad:?}");
    }
}

#[test]
fn a_start_by_a_link_takes_nothing_else() {
    let args = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    assert_eq!(link_launch(&args(&["+chimera:onboard", &link()])), Some(Some(link())));
    // A link that broke out of its quotes is one malformed link.
    assert_eq!(link_launch(&args(&["+chimera:onboard", &link(), "+exec", "evil.cfg"])), Some(None));
    assert_eq!(link_launch(&args(&["+chimera:onboard"])), Some(None));
    // Any other command line is not a link's.
    assert_eq!(link_launch(&args(&["+map", "de_dust2", "+chimera:onboard", &link()])), None);
    assert_eq!(link_launch(&[]), None);
}

fn handler(exe: &str) -> Handler {
    Handler { exe: PathBuf::from(exe), name: "Ultimate Gun Game".into() }
}

/// Windows: HKCU's keys through `reg.exe`, written once, again when the
/// program moved.
#[test]
fn windows_registers_under_hkcu_once_and_after_a_move() {
    let store: Arc<Mutex<HashMap<String, String>>> = Arc::default();
    let calls: Arc<Mutex<Vec<Vec<String>>>> = Arc::default();
    let (s, c) = (store.clone(), calls.clone());
    let reg = RegExe {
        run: Box::new(move |args| {
            c.lock().unwrap().push(args.to_vec());
            let mut s = s.lock().unwrap();
            match args[0].as_str() {
                "query" => s
                    .get(&format!("{}|", args[1]))
                    .map(|v| format!("\r\n{}\r\n    (Default)    REG_SZ    {v}\r\n\r\n", args[1]))
                    .ok_or_else(|| "not found".to_string()),
                "add" => {
                    let name = if args[2] == "/ve" { String::new() } else { args[3].clone() };
                    let d = args.iter().position(|a| a == "/d").unwrap();
                    s.insert(format!("{}|{name}", args[1]), args[d + 1].clone());
                    Ok(String::new())
                }
                _ => Err("?".into()),
            }
        }),
    };
    let h = handler(r"C:\Games\Chimera Party\ultimate_gun_game.exe");
    assert_eq!(reg.current(), None);
    assert!(ensure(&reg, &h).unwrap(), "a first launch writes");
    let s = store.lock().unwrap().clone();
    assert_eq!(s[r"HKCU\Software\Classes\chimera-hub|URL Protocol"], "");
    assert_eq!(
        s[r"HKCU\Software\Classes\chimera-hub\shell\open\command|"],
        r#""C:\Games\Chimera Party\ultimate_gun_game.exe" +chimera:onboard "%1""#
    );
    assert!(calls.lock().unwrap().iter().all(|a| a.first().is_some_and(|c| c == "query" || c == "add")));
    assert!(calls.lock().unwrap().iter().filter(|a| a[0] == "add").all(|a| a[1].starts_with(r"HKCU\Software\Classes\chimera-hub")));
    calls.lock().unwrap().clear();
    assert!(!ensure(&reg, &h).unwrap(), "the same program: nothing written");
    assert_eq!(calls.lock().unwrap().as_slice(), &[reg_query()]);
    assert!(ensure(&reg, &handler(r"D:\Moved\ultimate_gun_game.exe")).unwrap(), "moved: written again");
    assert_eq!(parse_reg_query("nothing here"), None);
}

/// Linux: the desktop entry and the default in `mimeapps.list`, under
/// temporary XDG folders, the list's other lines kept.
#[test]
fn linux_writes_a_desktop_entry_and_the_default() {
    let root = scratch("xdg");
    let xdg = Xdg { data_home: root.join("data"), config_home: root.join("config") };
    std::fs::create_dir_all(&xdg.config_home).unwrap();
    std::fs::write(
        xdg.mimeapps_path(),
        "[Default Applications]\ntext/html=firefox.desktop\nx-scheme-handler/chimera-hub=other.desktop\n\n[Added Associations]\nimage/png=viewer.desktop;\n",
    )
    .unwrap();
    let h = handler("/opt/chimera party/ultimate_gun_game");
    assert_eq!(xdg.current(), None, "another program's");
    assert!(ensure(&xdg, &h).unwrap());
    let list = std::fs::read_to_string(xdg.mimeapps_path()).unwrap();
    assert_eq!(default_in(&list).as_deref(), Some("chimera-hub.desktop"));
    assert!(list.contains("text/html=firefox.desktop") && list.contains("image/png=viewer.desktop;") && !list.contains("other.desktop"));
    let entry = std::fs::read_to_string(xdg.desktop_path()).unwrap();
    assert!(entry.contains("MimeType=x-scheme-handler/chimera-hub;"), "{entry}");
    assert!(entry.contains(r#"Exec="/opt/chimera party/ultimate_gun_game" +chimera:onboard %u"#), "{entry}");
    assert!(entry.contains("Path=/opt/chimera party\n") && entry.contains("NoDisplay=true"));
    assert!(!ensure(&xdg, &h).unwrap(), "registered already");
    assert!(ensure(&xdg, &handler("/opt/moved/ultimate_gun_game")).unwrap(), "moved");
    // An empty list gets the section.
    assert_eq!(with_default(""), "[Default Applications]\nx-scheme-handler/chimera-hub=chimera-hub.desktop\n");
    // Quotes, dollars, backslashes and percents in a path stay one argument.
    assert_eq!(desktop_exec(std::path::Path::new("/a\"b$c%d")), r#""/a\\"b\\$c%%d" +chimera:onboard %u"#);
    let _ = std::fs::remove_dir_all(&root);
}

/// The inbox: a running game's hold is seen, a link handed in is taken
/// once (and checked again), junk is dropped, a crashed game's lock goes.
#[test]
fn a_link_is_handed_to_the_running_game() {
    let dir = scratch("inbox");
    let inbox = LinkInbox::at(&dir);
    assert!(!inbox.running(), "no game");
    let hold = inbox.hold().unwrap();
    assert!(inbox.running(), "a game holds it");
    inbox.hand(&link()).unwrap();
    std::fs::write(dir.join("junk.link"), "chimera-hub://x:1?code=no; rm -rf").unwrap();
    assert_eq!(inbox.take(), vec![link()]);
    assert!(inbox.take().is_empty(), "taken once");
    drop(hold);
    assert!(!inbox.running(), "the game ended");
    // A lock file nobody holds (a crashed game's) is cleared.
    std::fs::write(dir.join("running-999999.lock"), "").unwrap();
    assert!(!inbox.running());
    assert!(!dir.join("running-999999.lock").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

/// In the app: a link handed to the inbox shows up in `OpenedLinks`; with
/// no registry nothing is written.
#[test]
fn the_running_app_takes_handed_links() {
    let dir = scratch("app");
    let inbox = LinkInbox::at(&dir);
    let mut sim = Sim::with(SimBasePlugins, |app| {
        app.add_plugins(LinkHandlerPlugin);
    });
    let hold = inbox.hold().ok();
    // Handed before the app's first look (it looks at once, then every
    // `POLL_SECS`).
    LinkInbox::at(&dir).hand(&link()).unwrap();
    sim.app.insert_resource(LinkSetup { registry: None, handler: None, inbox: Some(inbox.clone()), hold, registered: false });
    sim.ticks(1);
    assert_eq!(sim.app.world().resource::<OpenedLinks>().0, vec![link()]);
    drop(sim);
    let _ = std::fs::remove_dir_all(&dir);
}
